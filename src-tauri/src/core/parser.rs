use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct TurnNotification {
    pub character_name: String,
}

#[derive(Debug, Clone)]
pub struct GroupInviteNotification {
    /// The character that received the invite (window to focus).
    pub receiver_name: String,
    /// The character that sent the invite.
    pub inviter_name: String,
}

#[derive(Debug, Clone)]
pub struct TradeRequest {
    /// The character that received the trade request (window to focus).
    pub receiver_name: String,
    /// The character that initiated the trade.
    pub requester_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateMessage {
    pub receiver_name: String,
    pub sender_name: String,
    pub message: String,
}

/// What kind of game event a notification represents.
#[derive(Debug, Clone)]
pub enum GameEvent {
    Turn(TurnNotification),
    GroupInvite(GroupInviteNotification),
    Trade(TradeRequest),
    PrivateMessage(PrivateMessage),
}

static RE_TURN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:C'est à (.+?) de jouer|(.+?) 's turn to play|le toca jugar a (.+?))$")
        .unwrap()
});

static RE_GROUP_INVITE: LazyLock<Regex> = LazyLock::new(|| {
    // Group 1: FR/ES (name at start), Group 2: EN (name after "join")
    Regex::new(r"(?i)^(?:(.+?) (?:t'invite à rejoindre son groupe|te invita a unirte a su grupo)|You are invited to join (.+?)'s group)").unwrap()
});

static RE_TRADE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(.+?) (?:te propose de faire un échange|offers a trade|te propone realizar un intercambio)").unwrap()
});

static RE_PM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:desde|de|from) (.+?) : (.+)$").unwrap());

static RE_HTML_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<a\s[^>]*>([^<]*)</a>"#).unwrap());

static RE_HTML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

static RE_DOFUS_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?) - Dofus Retro v[\d.]+$").unwrap());

/// Replace `<a href="...">text</a>` with just `text`, then strip remaining tags.
/// Dofus wraps items in `[...]` already so we preserve those outer brackets.
pub fn clean_html(text: &str) -> String {
    let step1 = RE_HTML_LINK.replace_all(text, "$1");
    let step2 = RE_HTML_TAG.replace_all(&step1, "");
    step2.trim().to_string()
}

pub fn parse_turn_notification(text: &str) -> Option<TurnNotification> {
    let text = text.trim();
    if let Some(caps) = RE_TURN.captures(text) {
        // Groups: 1=FR, 2=EN, 3=ES — exactly one will be non-empty
        let name = [1, 2, 3]
            .iter()
            .filter_map(|&i| caps.get(i))
            .map(|m| m.as_str().trim())
            .find(|s| !s.is_empty())?;
        return Some(TurnNotification {
            character_name: name.to_string(),
        });
    }
    None
}

fn parse_group_invite(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(caps) = RE_GROUP_INVITE.captures(text) {
        // Group 1: FR/ES name, Group 2: EN name
        let name = [1, 2]
            .iter()
            .filter_map(|&i| caps.get(i))
            .map(|m| m.as_str().trim())
            .find(|s| !s.is_empty())?;
        return Some(name.to_string());
    }
    None
}

fn parse_trade(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(caps) = RE_TRADE.captures(text) {
        let requester = caps[1].trim();
        if !requester.is_empty() {
            return Some(requester.to_string());
        }
    }
    None
}

fn parse_pm(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    if let Some(caps) = RE_PM.captures(text) {
        let sender = caps[1].trim();
        let message = caps[2].trim();
        if !sender.is_empty() && !message.is_empty() {
            return Some((sender.to_string(), clean_html(message)));
        }
    }
    None
}

fn extract_character_from_title(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(caps) = RE_DOFUS_TITLE.captures(text) {
        let name = caps[1].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Parse all segments and return the first game event found.
/// Priority: turn > group invite > trade > PM
pub fn parse_game_event(segments: &[String]) -> Option<GameEvent> {
    // 1. Turn notifications — iterate in reverse so the specific body segment is matched
    //    before the earlier combined segment (which also ends with the turn suffix/pattern).
    for segment in segments.iter().rev() {
        if let Some(turn) = parse_turn_notification(segment) {
            return Some(GameEvent::Turn(turn));
        }
    }

    // Extract receiver (character name from title) once -- shared by invite/trade/PM.
    // Iterate in reverse because specific segments (title, body) come last;
    // the earlier combined segment would produce false matches.
    let mut receiver: Option<String> = None;
    for segment in segments.iter().rev() {
        if receiver.is_none() {
            if let Some(name) = extract_character_from_title(segment) {
                receiver = Some(name);
            }
        }
    }

    let receiver_name = match &receiver {
        Some(r) => r.clone(),
        None => {
            // No Dofus title segment found -- try combined text fallback for turn
            let combined = segments.join(" ");
            return parse_turn_notification(&combined).map(GameEvent::Turn);
        }
    };

    // 2. Group invite
    for segment in segments.iter().rev() {
        if let Some(inviter) = parse_group_invite(segment) {
            return Some(GameEvent::GroupInvite(GroupInviteNotification {
                receiver_name,
                inviter_name: inviter,
            }));
        }
    }

    // 3. Trade request
    for segment in segments.iter().rev() {
        if let Some(requester) = parse_trade(segment) {
            return Some(GameEvent::Trade(TradeRequest {
                receiver_name,
                requester_name: requester,
            }));
        }
    }

    // 4. Private message
    for segment in segments.iter().rev() {
        if let Some((sender, message)) = parse_pm(segment) {
            return Some(GameEvent::PrivateMessage(PrivateMessage {
                receiver_name,
                sender_name: sender,
                message,
            }));
        }
    }

    // Fallback: try combined text for turn
    let combined = segments.join(" ");
    parse_turn_notification(&combined).map(GameEvent::Turn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cest_a_jouer() {
        let result = parse_turn_notification("C'est à Craette de jouer");
        assert!(result.is_some());
        assert_eq!(result.unwrap().character_name, "Craette");
    }

    #[test]
    fn test_parse_cest_a_jouer_complex_name() {
        let result = parse_turn_notification("C'est à My-Char_123 de jouer");
        assert!(result.is_some());
        assert_eq!(result.unwrap().character_name, "My-Char_123");
    }

    #[test]
    fn test_parse_cest_a_jouer_case_insensitive() {
        let result = parse_turn_notification("c'est à TestChar de jouer");
        assert!(result.is_some());
        assert_eq!(result.unwrap().character_name, "TestChar");
    }

    #[test]
    fn test_parse_turn_segments() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Craette - Dofus Retro v1.47.20, C'est à Craette de jouer".to_string(),
            "Craette - Dofus Retro v1.47.20".to_string(),
            "C'est à Craette de jouer".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::Turn(_))));
        if let Some(GameEvent::Turn(t)) = result {
            assert_eq!(t.character_name, "Craette");
        }
    }

    #[test]
    fn test_parse_group_invite_segments() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, Testy t'invite à rejoindre son groupe.\nAcceptes-tu ?".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "Testy t'invite à rejoindre son groupe.\nAcceptes-tu ?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::GroupInvite(_))));
        if let Some(GameEvent::GroupInvite(g)) = result {
            assert_eq!(g.receiver_name, "Kura-noire");
            assert_eq!(g.inviter_name, "Testy");
        }
    }

    #[test]
    fn test_parse_trade_segments() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, Testy te propose de faire un échange.\nAcceptes-tu ?".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "Testy te propose de faire un échange.\nAcceptes-tu ?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::Trade(_))));
        if let Some(GameEvent::Trade(t)) = result {
            assert_eq!(t.receiver_name, "Kura-noire");
            assert_eq!(t.requester_name, "Testy");
        }
    }

    #[test]
    fn test_parse_pm_with_html() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, de Testy : [<a href=\"asfunction:onHref,ShowItemViewer,1\">Clef du Donjon d'Incarnam</a>]".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "de Testy : [<a href=\"asfunction:onHref,ShowItemViewer,1\">Clef du Donjon d'Incarnam</a>]".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::PrivateMessage(_))));
        if let Some(GameEvent::PrivateMessage(pm)) = result {
            assert_eq!(pm.receiver_name, "Kura-noire");
            assert_eq!(pm.sender_name, "Testy");
            assert_eq!(pm.message, "[Clef du Donjon d'Incarnam]");
        }
    }

    #[test]
    fn test_parse_pm_plain_text() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, de Testy : salut ca va ?".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "de Testy : salut ca va ?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::PrivateMessage(_))));
        if let Some(GameEvent::PrivateMessage(pm)) = result {
            assert_eq!(pm.receiver_name, "Kura-noire");
            assert_eq!(pm.sender_name, "Testy");
            assert_eq!(pm.message, "salut ca va ?");
        }
    }

    #[test]
    fn test_clean_html_link() {
        assert_eq!(
            clean_html(
                "[<a href=\"asfunction:onHref,ShowItemViewer,1\">Clef du Donjon d'Incarnam</a>]"
            ),
            "[Clef du Donjon d'Incarnam]"
        );
    }

    #[test]
    fn test_clean_html_plain() {
        assert_eq!(clean_html("hello world"), "hello world");
    }

    #[test]
    fn test_parse_english_turn_segments() {
        // Regression: combined segment ends with " 's turn to play" too — must not extract the whole combined string as character name
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Rave-ll - Dofus Retro v1.47.21, Rave-ll 's turn to play".to_string(),
            "Rave-ll - Dofus Retro v1.47.21".to_string(),
            "Rave-ll 's turn to play".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::Turn(_))));
        if let Some(GameEvent::Turn(t)) = result {
            assert_eq!(t.character_name, "Rave-ll");
        }
    }

    #[test]
    fn test_parse_english_turn_new() {
        let result = parse_turn_notification("Craette 's turn to play");
        assert!(result.is_some());
        assert_eq!(result.unwrap().character_name, "Craette");
    }

    #[test]
    fn test_parse_spanish_turn() {
        let result = parse_turn_notification("le toca jugar a Craette");
        assert!(result.is_some());
        assert_eq!(result.unwrap().character_name, "Craette");
    }

    #[test]
    fn test_parse_english_group_invite() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, You are invited to join Testy's group. Do you accept?".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "You are invited to join Testy's group. Do you accept?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::GroupInvite(_))));
        if let Some(GameEvent::GroupInvite(g)) = result {
            assert_eq!(g.receiver_name, "Kura-noire");
            assert_eq!(g.inviter_name, "Testy");
        }
    }

    #[test]
    fn test_parse_spanish_group_invite() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, Testy te invita a unirte a su grupo. ¿Deseas aceptar?".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "Testy te invita a unirte a su grupo. ¿Deseas aceptar?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::GroupInvite(_))));
        if let Some(GameEvent::GroupInvite(g)) = result {
            assert_eq!(g.receiver_name, "Kura-noire");
            assert_eq!(g.inviter_name, "Testy");
        }
    }

    #[test]
    fn test_parse_english_trade() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, Testy offers a trade. Do you accept?"
                .to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "Testy offers a trade. Do you accept?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::Trade(_))));
        if let Some(GameEvent::Trade(t)) = result {
            assert_eq!(t.receiver_name, "Kura-noire");
            assert_eq!(t.requester_name, "Testy");
        }
    }

    #[test]
    fn test_parse_spanish_trade() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, Testy te propone realizar un intercambio. ¿Deseas aceptar?".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "Testy te propone realizar un intercambio. ¿Deseas aceptar?".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::Trade(_))));
        if let Some(GameEvent::Trade(t)) = result {
            assert_eq!(t.receiver_name, "Kura-noire");
            assert_eq!(t.requester_name, "Testy");
        }
    }

    #[test]
    fn test_parse_spanish_pm() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, desde Testy : hola".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "desde Testy : hola".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::PrivateMessage(_))));
        if let Some(GameEvent::PrivateMessage(pm)) = result {
            assert_eq!(pm.receiver_name, "Kura-noire");
            assert_eq!(pm.sender_name, "Testy");
            assert_eq!(pm.message, "hola");
        }
    }

    #[test]
    fn test_parse_english_pm() {
        let segments = vec![
            "Notification Center".to_string(),
            "Dofus Retro, Kura-noire - Dofus Retro v1.47.20, from Testy : hello there".to_string(),
            "Kura-noire - Dofus Retro v1.47.20".to_string(),
            "from Testy : hello there".to_string(),
        ];
        let result = parse_game_event(&segments);
        assert!(matches!(result, Some(GameEvent::PrivateMessage(_))));
        if let Some(GameEvent::PrivateMessage(pm)) = result {
            assert_eq!(pm.receiver_name, "Kura-noire");
            assert_eq!(pm.sender_name, "Testy");
            assert_eq!(pm.message, "hello there");
        }
    }

    #[test]
    fn test_parse_unrelated() {
        assert!(parse_turn_notification("Some random notification").is_none());
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_turn_notification("").is_none());
    }
}
