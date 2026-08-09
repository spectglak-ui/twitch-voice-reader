//! Détection automatique de langue pour la sélection dynamique de voix.
//!
//! ## Choix technique : `whatlang` plutôt que `lingua-rs`
//!
//! | Critère | whatlang | lingua-rs |
//! |---|---|---|
//! | Vitesse sur texte court (message de chat) | Très rapide (n-grammes légers) | Plus lent (modèles statistiques plus lourds) |
//! | Taille binaire ajoutée | Minime | Plusieurs Mo de modèles embarqués |
//! | Précision sur texte très court (<10 mots) | Correcte mais faillible | Meilleure, mais coût disproportionné ici |
//!
//! Les messages de chat Twitch sont courts (souvent < 15 mots) : la
//! précision marginale de `lingua-rs` ne justifie pas son coût en taille et
//! en latence pour un usage temps réel. `whatlang` est utilisé avec un
//! seuil de confiance (`is_reliable()`) : en dessous, on retombe sur la
//! voix par défaut plutôt que de risquer une détection erronée.

use whatlang::{detect, Lang};

/// Codes ISO 639-1 supportés par la configuration `language_voice_map`.
pub fn detect_language_code(text: &str) -> Option<&'static str> {
    let info = detect(text)?;
    if !info.is_reliable() {
        return None;
    }
    match info.lang() {
        Lang::Fra => Some("fr"),
        Lang::Eng => Some("en"),
        Lang::Spa => Some("es"),
        Lang::Deu => Some("de"),
        Lang::Ita => Some("it"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_french() {
        assert_eq!(
            detect_language_code("Bonjour tout le monde, comment allez-vous aujourd'hui ?"),
            Some("fr")
        );
    }

    #[test]
    fn detects_english() {
        assert_eq!(
            detect_language_code("Hello everyone, how are you doing today my friends?"),
            Some("en")
        );
    }
}
