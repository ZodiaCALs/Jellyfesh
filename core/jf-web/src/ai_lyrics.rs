//! AI Lyrics in Jellyfesh Web (ADR-0019), as the core decides it (`jf-core/src/ai_lyrics.rs`, the
//! reference): what making lyrics needs first, from the AI Lyrics plugin's plan, Pro and the
//! origins the person allowed audio to go to. Jellyfesh Web has no on-device recognition and no
//! speech models, so it makes lyrics only in the online mode.

use jf_plugin_model::{MakerMode, MakerNeed, MakerPlan};
use serde::{Deserialize, Serialize};

/// What making lyrics needs first (the core's `AiLyricsNeed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AiLyricsNeed {
    /// Ready.
    Nothing,
    /// Jellyfesh Pro is inactive: the Pro sheet.
    Pro,
    /// The online mode needs the person's API key: AI Lyrics settings (its Sign In).
    SignIn,
    /// Ask "Send This Song to {origin}?" first.
    Consent { origin: String },
    /// Making lyrics cannot work as set up; the text says why.
    Unavailable { reason: String },
}

/// The plugin's plan as the page has it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanState {
    /// Being asked.
    Asking,
    /// The plugin could not say (the error).
    Failed(String),
    Ready(MakerPlan),
}

/// A consent to send audio: this plugin, to this origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioConsent {
    pub plugin: String,
    pub origin: String,
}

/// What making lyrics needs now (the core's `need` without models or a recogniser).
pub fn need(plugin: Option<&str>, pro: bool, plan: &PlanState, consents: &[AudioConsent]) -> AiLyricsNeed {
    let Some(plugin) = plugin else {
        return AiLyricsNeed::Unavailable {
            reason: "no AI Lyrics plugin is installed and on".into(),
        };
    };
    if !pro {
        return AiLyricsNeed::Pro;
    }
    let plan = match plan {
        PlanState::Asking => {
            return AiLyricsNeed::Unavailable {
                reason: "AI Lyrics is getting ready".into(),
            };
        }
        PlanState::Failed(reason) => {
            return AiLyricsNeed::Unavailable {
                reason: reason.clone(),
            };
        }
        PlanState::Ready(plan) => plan,
    };
    match &plan.need {
        MakerNeed::SignIn => AiLyricsNeed::SignIn,
        MakerNeed::Unavailable(reason) => AiLyricsNeed::Unavailable { reason: reason.clone() },
        MakerNeed::Model(_) => AiLyricsNeed::Unavailable {
            reason: "making lyrics on this device needs the Jellyfesh app; choose the online mode in AI Lyrics settings"
                .into(),
        },
        MakerNeed::Nothing => match (plan.mode, &plan.upload_to) {
            (MakerMode::OnDevice, _) => AiLyricsNeed::Unavailable {
                reason: "making lyrics on this device needs the Jellyfesh app; choose the online mode in AI Lyrics settings"
                    .into(),
            },
            (MakerMode::Online, Some(origin))
                if !consents.iter().any(|c| c.plugin == plugin && &c.origin == origin) =>
            {
                AiLyricsNeed::Consent { origin: origin.clone() }
            }
            _ => AiLyricsNeed::Nothing,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(mode: MakerMode, need: MakerNeed) -> PlanState {
        PlanState::Ready(MakerPlan {
            mode,
            engine: "Whisper".into(),
            upload_to: Some("https://api.example".into()),
            need,
            automatic: false,
        })
    }

    #[test]
    fn needs_follow_the_core() {
        let online = plan(MakerMode::Online, MakerNeed::Nothing);
        assert!(matches!(
            need(None, true, &online, &[]),
            AiLyricsNeed::Unavailable { .. }
        ));
        assert_eq!(need(Some("ai"), false, &online, &[]), AiLyricsNeed::Pro);
        assert!(matches!(
            need(Some("ai"), true, &PlanState::Asking, &[]),
            AiLyricsNeed::Unavailable { .. }
        ));
        assert_eq!(
            need(Some("ai"), true, &PlanState::Failed("offline".into()), &[]),
            AiLyricsNeed::Unavailable {
                reason: "offline".into()
            }
        );
        assert_eq!(
            need(Some("ai"), true, &plan(MakerMode::Online, MakerNeed::SignIn), &[]),
            AiLyricsNeed::SignIn
        );
        assert_eq!(
            need(Some("ai"), true, &online, &[]),
            AiLyricsNeed::Consent {
                origin: "https://api.example".into()
            }
        );
        let allowed = [AudioConsent {
            plugin: "ai".into(),
            origin: "https://api.example".into(),
        }];
        assert_eq!(need(Some("ai"), true, &online, &allowed), AiLyricsNeed::Nothing);
        let other = [AudioConsent {
            plugin: "other".into(),
            origin: "https://api.example".into(),
        }];
        assert!(
            matches!(
                need(Some("ai"), true, &online, &other),
                AiLyricsNeed::Consent { .. }
            ),
            "per plugin"
        );
    }

    #[test]
    fn on_the_device_needs_the_app() {
        for state in [
            plan(MakerMode::OnDevice, MakerNeed::Nothing),
            plan(MakerMode::OnDevice, MakerNeed::Model("base".into())),
        ] {
            assert!(matches!(
                need(Some("ai"), true, &state, &[]),
                AiLyricsNeed::Unavailable { .. }
            ));
        }
    }

    #[test]
    fn plan_states_read_from_the_pages_json() {
        let state: PlanState = serde_json::from_str(r#"{"failed": "no"}"#).unwrap();
        assert!(matches!(state, PlanState::Failed(_)));
        let state: PlanState = serde_json::from_str(r#""asking""#).unwrap();
        assert!(matches!(state, PlanState::Asking));
    }
}
