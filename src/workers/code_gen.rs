use codegen::{Block, Function, Impl, Scope, Struct};
use models::{ColumnDef};
use std::fs;
use std::fs::File;
use std::io::Error;
use std::io::prelude::*;
use std::path::Path;

pub struct CodeGen;

impl CodeGen {
    
    pub fn generate_handler(name: &str) -> Function {
        let mut myfn = Function::new(&name.to_lowercase());
        myfn
            .arg("req", "HttpRequest<State>")
            .ret("impl Future<Item=HttpResponse, Error=Error>")
            .line(&format!("use actors::{}::{};\n", name.to_lowercase(), name))
            .line(format!("\treq.state().db.send({}Msg{{page_num: 1}})", name))
            .line("\t\t.from_err()");
        let mut and_then_block = Block::new(".and_then(|res| ");
        let mut match_block = Block::new("\tmatch res ");
        match_block.line("Ok(i) => Ok(HttpResponse::Ok().json(i)),");
        
        let mut error_block = Block::new("Err(e) => ");
        error_block.line(&format!("eprintln!(\"get_{} error: {{}}]\",e);", name.to_lowercase()));
        error_block.line("Err(HttpResponse::InternalServerError().into())");
        match_block.push_block(error_block);
        and_then_block.push_block(match_block);
        myfn.push_block(and_then_block);

        myfn
    }

    pub fn generate_struct(name: &str, columns: &Vec<ColumnDef>) -> String {
        let mut scope = Scope::new();
        let mut my_model = Struct::new(name);
        
        if columns.len() > 0 {
            my_model
                .derive("Debug")
                .derive("Deserialize")
                .derive("Serialize");    
        }

        my_model.vis("pub");
        for c in columns.into_iter() {
            my_model.field(&c.name.to_lowercase(), c.data_type.string());
        }
        
        scope.push_struct(my_model);
        
        scope.to_string()
    }

    pub fn create_handler_actor(struct_meta: &(String, Vec<ColumnDef>)) -> String {
        let mut scope = Scope::new();
        let struct_name = &struct_meta.0;
        
        for (u0, u1) in vec![("actix", "prelude::*"), ("db", "DB"), ("models", struct_name), ("super","db_actor::DbExecutor")] {
            scope.import(u0, u1);
        }

        // Creating the message struct
        let msg_struct_name = &format!("{}Msg", struct_name);
        let mut msg_struct = Struct::new(msg_struct_name);
        msg_struct.doc(&format!("// Message for returning a paged list of {} records", struct_name));
        msg_struct.field("page_num", "u32");
        scope.push_struct(msg_struct);

        // impl for Message on the struct 
        let mut msg_impl = Impl::new(&format!("{}Msg", struct_name));
        msg_impl.impl_trait("Message");
        msg_impl.associate_type("Result", &format!("Result<{}, String>", struct_name));
        scope.push_impl(msg_impl);
        
        // This is for the Handler for the DbExecutor
        // impl Handler<Conspiracies> for DbExecutor {
        let mut handler_impl = Impl::new("DbExecutor");
        handler_impl.impl_trait(&format!("Handler<{}>", struct_name));
        handler_impl.associate_type("Result", &format!("Result<Vec<{}>, String>;", struct_name));

        let mut impl_func = Function::new("handle");
        impl_func.arg_mut_self();
        impl_func.arg("msg", msg_struct_name);
        impl_func.arg("_", "&mut Self::Context");
        impl_func.ret("Self::Result");
        impl_func.line(&format!("\tDB::get_{}(&self.0, msg.page_num)", struct_name.to_lowercase()));
        
        handler_impl.push_fn(impl_func);
        scope.push_impl(handler_impl);
        
        scope.to_string()
    }

    pub fn generate_mod_file_contents(mod_names: &Vec<String>) -> String{
        let mut scope = Scope::new();

        for file_name in mod_names.iter() {
            scope.raw(&format!("pub mod {};", file_name.to_lowercase().replace(".rs", "")));
        }

        scope.to_string()
    }

    pub fn generate_mod_file(dir: &str) -> String {
        let scope = Scope::new();
        
        let dir_path = Path::new(dir);
        if dir_path.is_dir() {
            let paths = fs::read_dir(dir_path).unwrap();
            for dir_entry in paths {
                let path = dir_entry.unwrap().path();
                if path.is_file() { 
                    let path_str: String = path.display().to_string();
                    if !path_str.ends_with("rs") {
                        continue;
                    }
                    //scope.raw(&format!("pub mod {};", path.file_name()));
                    // &self.files.push(path_str);
                    // num_files += 1;
                }
            }
        }

        scope.to_string()
    }

    

    pub fn generate_db_actor() -> String {
        let mut scope = Scope::new();

        scope.import("actix::prelude", "*");
        scope.import("rusqlite", "Connection");
        scope.raw("pub struct DbExecutor(pub Connection);");
        
        let mut actor_trait = Impl::new("DbExecutor");
        actor_trait.impl_trait("Actor");
        actor_trait.associate_type("Context", "SyncContext<Self>");

        scope.push_impl(actor_trait);
    
        scope.to_string()
    }

    pub fn generate_webservice(db_path: String, entities: &Vec<String>) -> String {
        let mut scope = Scope::new();
        
        for use_stmt in vec![("actix", "{Addr,Syn}"), ("actix::prelude", "*"), ("actors::db_actor", "*"), ("actix_web", "http, App, AsyncResponder, HttpRequest, HttpResponse"),
                            ("actix_web::server", "HttpServer"), ("futures", "Future"), ("actix_web", "Error"), ("actix_web", "Json"), ("actix_web::middleware", "Logger"),
                            ("rusqlite", "Connection"), ("models", "*")] {
            scope.import(use_stmt.0, use_stmt.1);
        }
        
        let mut state_struct = Struct::new("State");
        state_struct
            .doc("This is state where we will store *DbExecutor* address.")
            .field("db", "Addr<Syn, DbExecutor>");
        scope.push_struct(state_struct);

        let mut handlers = Struct::new("RouteHandlers");
        handlers.doc("Used to implement all of the route handlers");
        scope.push_struct(handlers);

        let mut index_fn = Function::new("index");
        index_fn
                .arg("_req", "HttpRequest<State>")
                .ret("&'static str")
                .line("\"Put the next steps instructions here\"");

        let mut handler_impl = Impl::new("RouteHandlers");
        handler_impl.push_fn(index_fn);

        for ent in entities {
            // add the handler funciton creation call here
            handler_impl.push_fn(CodeGen::generate_handler(&ent));
        }
        scope.push_impl(handler_impl);
        scope.raw("");

        create_extern_create_defs() + &scope.to_string() + &create_main_fn(db_path, &entities)
    }

    pub fn write_code_to_file(dir_path: &str, file_name: &str, code: String) -> Result<String, Error> {

        match File::create(format!("{}/{}", dir_path, &file_name).to_lowercase()) {
            Ok(mut file) => {
                match file.write_all(&code.into_bytes()) {
                    Ok(_) => Ok(file_name.to_string()),
                    Err(e) => Err(e)
                }
            },
            Err(e) => Err(e)
        }
    }

    pub fn create_curl_script(output_dir: &str, entities: &Vec<String>) -> Result<String, Error> {
        let mut scope = Scope::new();
        scope.raw("#!/bin/bash\n");
        for ent in entities {
                let lower_ent = ent.to_lowercase().replace(".rs", "");
                scope.raw(&format!("curl http://localhost:8088/{}", lower_ent));
        }

        return CodeGen::write_code_to_file(output_dir, "curl_test.sh", scope.to_string().replace("\n\n", "\n"))    
    }

}

fn create_extern_create_defs() -> String {
    let mut extern_scope = Scope::new(); 
        for extern_crate in vec!["pub mod actors;\npub mod models;\n\n\nextern crate clap;", "extern crate dotenv;", "extern crate env_logger;", "extern crate actix;", 
                                "extern crate actix_web;", "extern crate rusqlite;", "extern crate futures;", "#[macro_use]", "extern crate serde_derive;"] {
            extern_scope.raw(extern_crate);
        }
        extern_scope.raw("\n");

        extern_scope.to_string().replace("\n\n", "\n")
}

fn create_main_fn(db_path: String, entities: &Vec<String>) -> String {
    let mut main_fn_scope = Scope::new();
        main_fn_scope.raw("fn main() {");
        main_fn_scope.raw("\tstd::env::set_var(\"RUST_LOG\", \"actix_web=info\");");
        main_fn_scope.raw("\tenv_logger::init();");
        main_fn_scope.raw("\tlet sys = actix::System::new(\"csv2api\");");

        main_fn_scope.raw("// Start 3 parallel db executors");
        main_fn_scope.raw("\tlet addr = SyncArbiter::start(3, || {");
        main_fn_scope.raw(&format!("\t    DbExecutor(Connection::open(\"{}\").unwrap())", db_path));
        main_fn_scope.raw("\t});");

        main_fn_scope.raw("\tHttpServer::new(move || {");
        main_fn_scope.raw("\t\tApp::with_state(State{db: addr.clone()})");
        main_fn_scope.raw("\t\t\t.middleware(Logger::default())");
        main_fn_scope.raw("\t\t\t.resource(\"/\", |r| r.method(http::Method::GET).f(RouteHandlers::index))");

        for ent in entities {
            let lower_ent = ent.to_lowercase();
            main_fn_scope.raw(&format!("\t\t\t.resource(\"/{}\", |r| r.method(http::Method::GET).f(RouteHandlers::{}))", lower_ent, lower_ent));
        }

        main_fn_scope.raw("\t})");
        main_fn_scope.raw("\t.bind(\"127.0.0.1:8088\").unwrap()");
        main_fn_scope.raw("\t.start();\n");
        main_fn_scope.raw("\tprintln!(\"Started http server: 127.0.0.1:8088\");");
        main_fn_scope.raw("\tlet _ = sys.run();");
        main_fn_scope.raw("}");

        main_fn_scope.to_string().replace("\n\n", "\n")
}


#[cfg(test)]
mod tests {
    use workers::code_gen::CodeGen;
    use models::{ColumnDef, DataTypes};
    use codegen::{Block, Formatter, Function, Impl, Scope, Struct};

    #[test]
    fn generate_mod_file() {
        let expected = "".to_string();
        let actual = CodeGen::generate_mod_file("./src/workers");

        assert_eq!(actual, expected);
    }

    #[test]
    fn create_handler_actor() {
        let expected_len = 490;
        let actual = CodeGen::create_handler_actor(&("my_actor".to_string(), vec![ColumnDef::new("my_col".to_string(), DataTypes::String)]));

        assert_eq!(actual.len(), expected_len);
    }

    #[test] 
    fn generate_struct() {
        let struct_def = "#[derive(Debug, Deserialize, Serialize)]\npub struct people {\n    name: String,\n    age: i64,\n    weight: f64,\n}".to_string();
        let cols: Vec<ColumnDef> = vec![ColumnDef::new("name".to_string(), DataTypes::String), ColumnDef::new("age".to_string(), DataTypes::I64), ColumnDef::new("weight".to_string(), DataTypes::F64)];
        assert_eq!(struct_def, CodeGen::generate_struct("people", &cols));
    }

    #[test] 
    fn generate_struct_with_no_columns() {
        let struct_def = "pub struct people;".to_string();
        let cols: Vec<ColumnDef> = vec![];
        assert_eq!(struct_def, CodeGen::generate_struct("people", &cols));
    }

    #[test]
    fn generate_db_actor() {
        let db_actor = "use actix::prelude::*;\nuse rusqlite::Connection;\n\npub struct DbExecutor(pub Connection);\n\nimpl Actor for DbExecutor {\n    type Context = SyncContext<Self>;\n}".to_string();
        assert_eq!(db_actor, CodeGen::generate_db_actor());
    }

    #[test]
    fn generate_webservice_main() {
        let expected = "pub mod actors;\npub mod models;\n\n extern crate clap;\nextern crate dotenv;\nextern crate env_logger;\nextern crate actix;\nextern crate actix_web;\nextern crate rusqlite;\nextern crate futures;\n#[macro_use]\nextern crate serde_derive;\n\nuse actix::{Addr,Syn};\nuse actix::prelude::*;\nuse actors::db_actor::*;\nuse actix_web::{http, App, AsyncResponder, HttpRequest, HttpResponse, Error, Json};\nuse actix_web::server::HttpServer;\nuse futures::Future;\nuse actix_web::middleware::Logger;\nuse rusqlite::Connection;\nuse models::*;\n\n/// This is state where we will store *DbExecutor* address.\nstruct State {\n       db: Addr<Syn, DbExecutor>,\n}\n\n/// Used to implementall of the route handlers\nstruct RouteHandlers;\n\nimpl RouteHandlers {\n    fn index(_req: HttpRequest<State>) -> &\'static str {\n    \"Put the next steps instructions here\"\n    }\n}\n\nfn main() {\n\tstd::env::set_var(\"RUST_LOG\", \"actix_web=info\");\n\tenv_logger::init();\n\tlet sys = actix::System::new(\"csv2api\");\n// Start 3 parallel db executors\n\tlet addr = SyncArbiter::start(3, || {\n\t     DbExecutor(Connection::open(\"test.db\").unwrap())\n\t});\n\tHttpServer::new(move || {\n\t\tApp::with_state(State{db: addr.clone()})\n\t\t\t.middleware(Logger::default())\n\t\t\t.resource(\"/\", |r| r.method(http::Method::GET).f(RouteHandlers::index))\n\t})\n\t.bind(\"127.0.0.1:8088\").unwrap()\n\t.start();\n\n\tprintln!(\"Started http server: 127.0.0.1:8088\");\n\tlet _ = sys.run();\n}".to_string();
        let actual = CodeGen::generate_webservice("test.db".to_string(),&vec![]);

        assert_eq!(actual.len(), expected.len());
    }

    #[test]
    fn generate_handler() {
        let expected_len = 536;

        let mut actual_scope = Scope::new();
        let mut actual_impl = Impl::new("test");
        let actual_fn = CodeGen::generate_handler("MyEntity");
        actual_impl.push_fn(actual_fn);
        actual_scope.push_impl(actual_impl);
        let actual = actual_scope.to_string();

        assert_eq!(actual.len(), expected_len);
    }
}