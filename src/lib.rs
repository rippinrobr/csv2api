extern crate barrel;
extern crate failure;
extern crate glob;
extern crate failure_derive;
extern crate postgres;
extern crate structopt;

pub mod cmd;
pub mod models;
pub mod parsers;
pub mod storage;

use std::path::PathBuf;
use structopt::StructOpt;
use crate::cmd::db;
use crate::models::{InputSource, ParsedContent};

/// All command line options/flags broken into their sub-commands
#[derive(Debug, StructOpt)]
#[structopt(name = "csv-to", about = "creates databases and code from CSV data")]
pub enum CsvTo {
    #[structopt(name = "db", about = "creates and loads a database from CSV file(s)")]
    Db {
        #[structopt(short = "f", parse(from_os_str), long = "files", help = "The CSV files to be processed, can be /path/to/files/ or a comma delimited string of paths")]
        files: Vec<PathBuf>,

        #[structopt(short = "d", parse(from_os_str), long = "directories", help = "The directories that contain CSV files to be processed, a comma delimited string of paths")]
        directories: Vec<PathBuf>,

        #[structopt(short = "t", long = "type", help = "The type of database to create, valid types are sqlite and postgres")]
        db_type: db::Types,

        #[structopt(short = "c", long = "connection-info", help = "Database connectivity information")]
        connection_info: String,

        #[structopt(short = "n", long = "name", help = "Name of the database to be created")]
        name: String,

        #[structopt(long = "drop-stores", help = "Drops tables/collections if the already exist")]
        drop_stores: bool,

        #[structopt(long = "no-headers", help = "The CSV file(s) have no column headers")]
        no_headers: bool
    }
}

// This trait is what all of the sub-commands will implement so they can have a common
// interface that the main can call into to start the csv_to logic started
pub trait App {
    fn run(&self) -> Result<ParsedContent, std::io::Error> ;
}

/// ConfigService is used to encapsulate the input from the user and allows each 'app' or sub-command
/// in csv-to to have access to the input without having to worry about parsing and gathering
pub trait ConfigService {
    /// Returns a Vec<InputSource> that represents all input files/sources
    fn get_input_sources(&self) -> Vec<InputSource>;
    /// Returns true if the input files have column headers, currently
    /// all files have them or none of them do
    fn has_headers(&self) -> bool;
    /// Returns true if tables/collections should be removed before
    /// loading the data
    fn should_drop_store(&self) -> bool;
}