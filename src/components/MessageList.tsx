import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import {
  StoredMessage,
  getMessages,
  clearMessages,
} from "../lib/commands";

function formatTime(epoch: number): string {
  const d = new Date(epoch * 1000);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function renderMessage(text: string) {
  const parts = text.split(/(\[[^\]]+\])/g);
  return parts.map((part, i) => {
    if (part.startsWith("[") && part.endsWith("]")) {
      return (
        <span key={i} className="font-bold">
          {part}
        </span>
      );
    }
    return <span key={i}>{part}</span>;
  });
}

function MessageList() {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<StoredMessage[]>([]);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getMessages().then(setMessages);

    const unlisten = listen<StoredMessage>("new-pm", (e) => {
      setMessages((prev) => [...prev, e.payload]);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleClear = async () => {
    await clearMessages();
    setMessages([]);
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          {t("messages.title")}
        </h2>
        {messages.length > 0 && (
          <button
            onClick={handleClear}
            className="text-xs px-2 py-1 rounded bg-gray-100 hover:bg-gray-200 dark:bg-gray-800 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 transition-colors"
          >
            {t("messages.clear")}
          </button>
        )}
      </div>

      {messages.length === 0 ? (
        <div className="text-center py-8 text-gray-400 dark:text-gray-600">
          <p className="text-sm">{t("messages.empty_title")}</p>
          <p className="text-xs mt-1">{t("messages.empty_desc")}</p>
        </div>
      ) : (
        <div className="space-y-0.5 font-mono">
          {messages.map((msg, i) => (
            <p
              key={`${msg.timestamp}-${i}`}
              className="text-[11px] leading-4 text-sky-600 dark:text-sky-400"
            >
              <span className="text-sky-800 dark:text-sky-600">[{formatTime(msg.timestamp)}]</span>{" "}
              {t("messages.from")} <span className="font-bold">{msg.sender}</span> : {renderMessage(msg.message)}
            </p>
          ))}
          <div ref={bottomRef} />
        </div>
      )}
    </div>
  );
}

export default MessageList;
