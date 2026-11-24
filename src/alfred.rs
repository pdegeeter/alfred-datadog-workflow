//! Script filter items for every keyword.
//!
//! Every actionable item (and each of its modifiers) carries an
//! [`Action`] encoded in its `arg`; the one connected Run Script decodes and
//! performs it. No `uid` is set: Alfred would otherwise re-sort items by its
//! own usage knowledge and override the order computed here (match quality,
//! frecency, state).

use powerpack::{Item, Key, Modifier};

use crate::action::Action;

/// macOS system icons, so the workflow ships no artwork of its own.
pub mod icons {
    const DIR: &str = "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources";

    fn system(name: &str) -> powerpack::Icon {
        powerpack::Icon::with_image(format!("{DIR}/{name}.icns"))
    }

    pub fn error() -> powerpack::Icon {
        system("AlertNoteIcon")
    }

    pub fn alert() -> powerpack::Icon {
        system("AlertStopIcon")
    }

    pub fn warning() -> powerpack::Icon {
        system("AlertCautionBadgeIcon")
    }

    pub fn unknown() -> powerpack::Icon {
        system("GenericQuestionMarkIcon")
    }

    pub fn dashboard() -> powerpack::Icon {
        system("GenericWindowIcon")
    }

    pub fn notebook() -> powerpack::Icon {
        system("GenericStationeryIcon")
    }

    pub fn service() -> powerpack::Icon {
        system("GenericNetworkIcon")
    }

    pub fn link() -> powerpack::Icon {
        system("GenericURLIcon")
    }

    pub fn document() -> powerpack::Icon {
        system("GenericDocumentIcon")
    }

    pub fn refresh() -> powerpack::Icon {
        system("Clock")
    }
}

/// Item shown in place of a list when something went wrong.
pub fn error_item(title: &str, message: &str) -> Item {
    Item::new(title)
        .subtitle(message)
        .valid(false)
        .icon(icons::error())
}

/// Item that runs `action` on <kbd>↩</kbd>.
pub fn action_item(title: impl Into<String>, subtitle: impl Into<String>, action: &Action) -> Item {
    Item::new(title).subtitle(subtitle).arg(action.to_arg())
}

/// Modifier that runs `action`.
pub fn modifier(keys: &[Key], subtitle: &str, action: &Action) -> Modifier {
    Modifier::new_multi(keys.iter().copied())
        .subtitle(subtitle)
        .arg(action.to_arg())
}
