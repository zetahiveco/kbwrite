use kbwrite_plugin::{export_plugin, Api, Plugin};

#[derive(Default)]
struct Flashcards;

impl Plugin for Flashcards {
    fn load(&mut self, api: &Api) {
        let _ = api.register_command("review", "Review flashcards");
        let _ = api.register_block(
            r#"{
                "id": "card",
                "title": "Flashcard",
                "description": "A card with a front and a back",
                "group": "Flashcards",
                "props": [
                    {"id": "front", "label": "Front"},
                    {"id": "back", "label": "Back"}
                ]
            }"#,
        );
    }

    fn command(&mut self, api: &Api, id: &str) {
        if id != "review" {
            return;
        }
        match api.open_dialog(
            r#"{
                "title": "Review flashcards",
                "message": "Answer the card, then check it. A plugin can read the vault and score this.",
                "fields": [
                    {"id": "answer", "label": "Answer", "kind": "text"},
                    {"id": "again", "label": "Show this card again", "kind": "checkbox", "value": "true"}
                ],
                "confirm": "Check",
                "cancel": "Close"
            }"#,
        ) {
            Ok(response) => api.log(&format!("Review finished: {response}")),
            Err(error) => api.log(&error),
        }
    }
}

export_plugin!(Flashcards);
