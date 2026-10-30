use indexmap::IndexMap;
use std::io::Write;


/*
The abstraction we have here provides a couple of wrapper methods which make it possible for code which is not dependent
on the hardware specific implementation to create web pages which return a fixed status and headers and provide a closure
to generate the output body.

Anything which requires conditional HTTP status codes or the like needs to be implemented using the implementation type directly.

I have made extensive efforts to refactor this into a hardware agnostic set of traits and an ESP32 specific back end
without success. Neither boxed trait objects (which require memory allocations on each request) or a solution based
on generic associated types (GAT) will work.

My conclusion is that this is not possible between the way the ESP Http server works and the limitations of the rust
type system.

I HAVE BEEN DOWN THIS RABBIT HOLE TWICE NOW.

NO ENTRY.

STOP.

GO BACK.
*/

pub enum HttpMethod {
    Get,
    Post,
}

pub trait HttpServerManager {
    fn handle(
        &mut self,
        uri: &str,
        method: HttpMethod,
        f: Box<dyn Fn(&mut dyn Write) -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()>;

    fn handle_post_form(
        &mut self,
        uri: &str,
        f: Box<
            dyn Fn(&mut dyn Write, IndexMap<String, String>) -> anyhow::Result<()>
                + Send,
        >,
    ) -> anyhow::Result<()>;

    fn handle_status(
        &mut self,
        uri: &str,
        method: HttpMethod,
        status: u16,
        message: Option<&'static str>,
        headers: &'static [(&'static str, &'static str)],
        f: Box<dyn Fn(&mut dyn Write) -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()>;
    
    fn init_common_pages(&mut self) -> anyhow::Result<()> {
        self.handle_status(
            "/main.css", 
            HttpMethod::Get, 
            200,
            Some("OK"),
            &[("Content-Type", "text/css")],
            Box::new(|resp| {
            resp.write(r#"
/* Dark, as the Marco Sparko app is, and for a phone first: one column, fields the width of the screen. */
:root {
  color-scheme: dark;
  --bg: #0f1116;
  --panel: #171a21;
  --border: #2e333d;
  --text: #e8eaed;
  --muted: #9aa6b2;
  --accent: #06b6d4;
  --problem: #e06c6c;
}
*, *::before, *::after { box-sizing: border-box; }
/* The hidden attribute, e.g. of Clear Password's tick box, beats the display of the rules below. */
[hidden] { display: none !important; }
html { -webkit-text-size-adjust: 100%; }
body { margin: 0; background: var(--bg); color: var(--text); font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; line-height: 1.4; }
.page { max-width: 560px; margin: 0 auto; padding: 16px; overflow-wrap: anywhere; }
h1 { font-size: 1.4rem; margin: 4px 0 16px; }
h2 { font-size: 1.1rem; margin: 0; color: var(--accent); }
p { color: var(--muted); }
a { color: var(--accent); }
.feature { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 14px; margin: 0 0 16px; }
.feature_head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
label { display: block; margin: 14px 0 4px; font-size: 0.85rem; color: var(--muted); }
/* 16px, so that a phone does not zoom in to the field. */
input, select { display: block; width: 100%; min-width: 0; padding: 10px; font-size: 16px; color: var(--text); background: var(--bg); border: 1px solid var(--border); border-radius: 6px; }
input:focus, select:focus { outline: none; border-color: var(--accent); }
input::placeholder { color: var(--muted); }
.checkbox { display: flex; align-items: center; gap: 10px; margin: 14px 0 4px; font-size: 1rem; color: var(--text); }
.feature_head .checkbox { margin: 0; font-size: 0.9rem; color: var(--muted); }
input[type="checkbox"] { display: inline-block; width: 20px; height: 20px; margin: 0; padding: 0; accent-color: var(--accent); flex: 0 0 auto; }
button { display: block; width: 100%; margin: 16px 0 0; padding: 12px; font-size: 1rem; color: var(--accent); background: #2a2a2f; border: 1px solid #404045; border-radius: 6px; cursor: pointer; }
button:hover, button:active { border-color: var(--accent); }
.note { margin: 4px 0 0; font-size: 0.85rem; }
.problems { margin: 0 0 16px; padding: 10px 14px; color: #ffd7d7; background: #3a1d1d; border: 1px solid var(--problem); border-radius: 8px; }
.problems ul { margin: 6px 0 0; padding-left: 20px; }
/* A field with a button or picker beside it, which wraps onto two lines if the screen is too narrow. */
.form_row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.form_row input[type="text"], .form_row input[type="password"] { flex: 1 1 10em; width: auto; }
.form_row button { flex: 0 0 auto; width: auto; margin: 0; padding: 10px 12px; white-space: nowrap; }
.form_row input[type="color"] { flex: 0 0 auto; width: 44px; height: 42px; padding: 2px; cursor: pointer; }
                        "#.as_bytes())?;
            Ok(())
        }))?;
        Ok(())
    }
}
