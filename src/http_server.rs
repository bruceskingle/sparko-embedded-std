use indexmap::IndexMap;
use std::io::Write;
use std::io::Read;


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

// pub trait Request : Read {
//     // type Resp: Response;

//     // fn into_ok_response(self) -> anyhow::Result<impl Response>;
//     // fn into_ok_response(self: Box<Self>) -> anyhow::Result<Box<dyn Response>>;
//     type Resp<'a>: Response
//     where
//         Self: 'a;

//     fn into_ok_response<'a>(self) -> anyhow::Result<Self::Resp<'a>>
//     where
//         Self: 'a;

// }

// pub trait Response: Write {

// }

// // pub type Handler = Box<dyn Fn(Box<dyn Request>) -> anyhow::Result<()> + Send + Sync + 'static>;
// pub type Handler =
//     Box<
//         dyn for<'r> Fn(
//                 Box<dyn Request + 'r>
//             ) -> anyhow::Result<()>
//         + Send
//         + Sync
//         + 'static
//     >;


// pub trait HttpServerManager {
//     // type Req: Request;
    

//     fn fn_handler(
//         &mut self,
//         uri: &str,
//         method: HttpMethod,
//         handler: Handler,
//     ) -> anyhow::Result<()>
//     ;
// }

// pub fn call_it(req: Box<dyn Request>) -> anyhow::Result<()> {
//     let mut resp = req.into_ok_response()?;

//     resp.write(b"Hello")?;

//     Ok(())
// }

// pub fn use_it(server: Box<&mut dyn HttpServerManager>)  -> anyhow::Result<()>{
//     server.fn_handler("/", HttpMethod::Get, Box::new(move |req| {
//         let mut resp = req.into_ok_response()?;

//         resp.write(b"Hello")?;
//         Ok(())
//     }))?;

//     Ok(())
// }

// pub struct HttpServerWrapper<S: HttpServerManager> {
//     server: S,
// }

// impl<S: HttpServerManager> HttpServerWrapper<S> {
//     pub fn new(server: S) -> Self {
//         HttpServerWrapper { server }
//     }
// }

// impl<S: HttpServerManager> HttpServerManager for HttpServerWrapper<S> {
//     fn handle<F>(
//         &mut self,
//         uri: &str,
//         method: HttpMethod,
//         f: F,
//     ) -> anyhow::Result<()>
//     where
//         F: for<'r> Fn(&mut dyn Response) -> anyhow::Result<()> + Send + 'static {
//         self.server.handle(uri, method, f)
//     }

//     fn handle_post_form<F>(
//         &mut self,
//         uri: &str,
//         f: F,
//     ) -> anyhow::Result<()>
//     where
//         F: for<'r> Fn(&mut dyn Response, IndexMap<String, String>) -> anyhow::Result<()> + Send + 'static {
//         self.server.handle_post_form(uri, f)
//     }

//     fn handle_status<F>(
//         &mut self,
//         uri: &str,
//         method: HttpMethod,
//         status: u16,
//         message: Option<&'static str>,
//         headers: &'static [(&'static str, &'static str)],
//         f: F,
//     ) -> anyhow::Result<()>
//     where
//         F: for<'r> Fn(&mut dyn Response) -> anyhow::Result<()> + Send + 'static {
//         self.server.handle_status(uri, method, status, message, headers, f)
//     }
// }

// pub struct Responder {
//     pub status: u16,
//     pub message: Option<&'static str>,
//     pub headers: &'static [(&'static str, &'static str)],
//     pub f: Box<dyn Fn(&mut dyn Response) -> anyhow::Result<()> + Send>,
// }

// pub struct Discriminator<T> {
//     pub status: u16,
//     pub message: Option<&'static str>,
//     pub headers: &'static [(&'static str, &'static str)],
//     pub discriminator: T,
// }

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

    // fn responder(
    //     &mut self,
    //     uri: &str,
    //     method: HttpMethod,
    //     responder: dyn Fn() -> anyhow::Result<Responder> + Send,
    // ) -> anyhow::Result<()>;

    // fn discriminator<T: Clone + 'static>(
    //     &mut self,
    //     uri: &str,
    //     method: HttpMethod,
    //     discriminator: Box<dyn Fn() -> anyhow::Result<Discriminator<T>> + Send>,
    //     f: Box<dyn Fn(&mut dyn Response, T) -> anyhow::Result<()> + Send>,
    // ) -> anyhow::Result<()>;
    
    fn init_common_pages(&mut self) -> anyhow::Result<()> {
        self.handle_status(
            "/main.css", 
            HttpMethod::Get, 
            200,
            Some("OK"),
            &[("Content-Type", "text/css")],
            Box::new(|resp| {
            resp.write(r#"
body { font-family: system-ui, -apple-system, BlinkMacSystemFont, sans-serif; margin: 0; padding: 0; background: #f7f7f7; }
.page { max-width: 480px; margin: 0 auto; padding: 18px; }
h1 { font-size: 1.5rem; margin-bottom: 1rem; }
label { display: block; margin: 12px 0 6px; font-weight: 600; }
input, select { width: 100%; padding: 10px 10px; border: 1px solid #ccc; border-radius: 8px; box-sizing: border-box; }
button { margin-top: 18px; width: 100%; padding: 12px; font-size: 1rem; border-radius: 10px; border: none; background: #007aff; color: #fff; }
button:active { background: #005bb5; }
                        "#.as_bytes())?;
            Ok(())
        }))?;
        Ok(())
    }
}

// type Handler: for<'r> Fn(dyn &mut Request) -> anyhow::Result<()> + Send + 'static;
