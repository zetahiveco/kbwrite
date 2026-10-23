use kbwrite_plugin::{export_plugin, Api, Plugin};

#[derive(Default)]
struct Hello;

impl Plugin for Hello {
    fn load(&mut self, api: &Api) {
        let _ = api.register_command("hello", "Write hello note");
    }

    fn command(&mut self, api: &Api, id: &str) {
        if id != "hello" {
            return;
        }
        match api.create_note("Hello from plugin", "Written by the Hello plugin.\n") {
            Ok(path) => api.log(&format!("Created {path}")),
            Err(error) => api.log(&error),
        }
    }
}

export_plugin!(Hello);
