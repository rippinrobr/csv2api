extern crate ansi_term;

pub mod config;

use std::path::Path;
use std::str::FromStr;
use ansi_term::Colour::{Green, Red, Yellow};
use indicatif::{ProgressBar, ProgressStyle};

use crate::ColumnDef;
use crate::parsers::InputService;
use crate::ConfigService;
use crate::storage::StorageService;
use crate::cache::{Cache, CacheType, DataDefinition, CacheService};

/// DbApp is used to manage the creation of the database
/// This app is used when the db sub-command is provided
pub struct DbApp<C,I,J,S>
where
    C: ConfigService,
    I: InputService,
    J: CacheService,
    S: StorageService,
{
    config_svc: C,
    input_svc: I,
    cache_svc: J,
    storage_svc: S,
}

impl<C,I,J,S> DbApp<C,I,J,S>
where
    C: ConfigService,
    I: InputService,
    J: CacheService,
    S: StorageService,
{
    /// creates an instance of the DbApp struct
    pub fn new(config_svc: C, input_svc: I, cache_svc: J, storage_svc: S) -> DbApp<C,I,J,S> {
        DbApp{
            config_svc,
            input_svc,
            cache_svc,
            storage_svc,
        }
    }

    /// execute the application logic
    pub fn run(self) -> Result<(), std::io::Error> {
        let inputs = self.config_svc.get_input_sources();
        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut results: Vec<DBResults> = Vec::new();

        // Setting up the cache system here. The second line determines if I'm supposed to save a
        // cache file and the third line is here so that if I'm saving a cache while working with
        // a single table so I only create one cache file
        let mut cache: Cache = Cache::new(self.config_svc.get_name(), CacheType::Db);
        let save_cache = self.config_svc.should_save_cache();
        let mut have_added_cache = false;

        let using_single_table = match self.config_svc.has_single_table() {
            Some(_) => true,
            None => false,
        };
        let keep_tables_delete_data = self.config_svc.should_delete_data();
        let mut need_to_create_single_table = using_single_table;

        let pbar = ProgressBar::new(inputs.len() as u64);
        pbar.set_style(ProgressStyle::default_bar()
            .template("{prefix:.cyan/blue} {msg} [{bar:40.cyan/blue}] {pos:>3/blue}/{len:3}files")
            .progress_chars("=> "));
        pbar.set_prefix("Processing");

        let mut num_files = 0;
        for input in inputs {
            pbar.set_message(&format!("{}", &input.location));
            match self.input_svc.parse(input) {
                Err(e) => errors.push(format!("parse error: {:?}", e)),
                Ok(mut pc) => {
                    if !&pc.errors.is_empty() {
                        errors.append(&mut pc.errors.clone());
                    }

                    if pc.records_parsed == 0 {
                        warnings.push(format!("the input source '{}' was not a CSV file or had no data.", pc.file_name));
                    }

                    pc.set_column_data_types();
                    pbar.set_prefix("Loading Data...");

                    let table_name = self.get_table_name(&pc.file_name);

                    // TODO: change this to be less hackie
                    if !keep_tables_delete_data {
                        if   !using_single_table || need_to_create_single_table {
                            if let Err(e) = self.storage_svc.create_store(table_name.clone(), pc.columns.clone(), self.config_svc.should_drop_store()) {
                                errors.push(format!("error while attempting to create '{}' table => {}", table_name, e));
                                continue;
                            }
                            need_to_create_single_table = false;
                        }
                    }

                    if keep_tables_delete_data {
                        if let Err(e) = self.storage_svc.delete_data_in_table(table_name.clone()) {
                            errors.push(format!("error will attempting to delete data from table '{}', error '{}'", table_name, e));
                            continue;
                        }
                    }
                    match self.store(table_name.clone(),
                                     pc.file_name.clone(),
                                     pc.records_parsed,
                                     pc.columns.clone(),
                                     pc.content.clone()) {
                            Ok(result) => results.push(result),
                            Err(e) => errors.push(format!("{}", e)),
                    }


                    // Todo: clean this up
                    // I want to only save cache when I have more than one table I'm storing
                    // data or if I am using a single table I have yet to add a data definition
                    // to the cache
                    if save_cache &&  ( !using_single_table || !have_added_cache) {
                        let data_def = DataDefinition::new(table_name.clone(), pc.columns.clone());
                        cache.add_data_definition(data_def);
                        have_added_cache = true
                    }
                    pbar.inc(1)
                }
            }
            num_files += 1;
        }
        pbar.finish_and_clear();

        // Pressing report
        self.display_report(results, errors, warnings, num_files, using_single_table);

        if save_cache {
            match self.cache_svc.write(cache) {
                Err(e) => eprintln!("{}", e),
                Ok(_) => (),
            }
        }

        Ok(())
    }

    fn display_report(&self, store_results: Vec<DBResults>, errors: Vec<String>, warnings: Vec<String>, num_files: u64, using_single_table: bool) {
        let processed_msg = format!("{} files processed", num_files);
        let num_errors = errors.len();

        let err_stmt = match num_errors == 0 {
            true =>  format!("{}", Green.bold().paint("0 errors")),
            false => format!("{}", Red.bold().paint(format!("{} Errors", num_errors)))
        };

        let warning_stmt = match warnings.len() == 0 {
            true =>  format!("{}", Green.bold().paint("0 warnings")),
            false => format!("{}", Yellow.bold().paint(format!("{} Warnings", num_errors)))
        };

        println!("\ncsv-to results");
        println!("-------------------");
        println!("{} / {} / {}", Green.bold().paint(processed_msg), err_stmt, warning_stmt);
        for r in store_results {
            match r.get_results(using_single_table) {
                Ok(msg) => println!("{}", msg),
                Err(msg) => println!("{}", Red.bold().paint(format!("{}", msg)))
            }
        }

        if num_errors > 0 {
            let err_msg =format!("\nError Details\n-------------");
            println!("{}", Red.bold().paint(err_msg));
            for e in errors {
                eprintln!("{}", e);
            }
        }

        if warnings.len() > 0 {
            let msg =format!("\nWarning Details\n-------------");
            println!("{}", Red.bold().paint(msg));
            for e in warnings {
                eprintln!("{}", e);
            }
        }
    }

    fn store(&self, name: String, file_name: String, records_parsed: usize, columns: Vec<ColumnDef>, content: Vec<csv::StringRecord>) -> Result<DBResults, failure::Error> {
        let insert_stmt = self.storage_svc.create_insert_stmt(name.clone(), columns.clone());

        match self.storage_svc.store_data( columns.clone(), content, insert_stmt) {
            Ok(records_inserted) => Ok(DBResults::new(name.clone(), file_name.to_string(), records_parsed, records_inserted)),
            Err(e) => Err(e)
        }
    }

    fn get_table_name(&self, file_path: &str) -> String {
        if let Some(table_name) = &self.config_svc.has_single_table() {
            return table_name.clone();
        }

        let name = String::from(Path::new(&file_path).file_name().unwrap().to_str().unwrap());
        let first_letter = name.trim_right_matches(".csv").chars().next().unwrap();
        name.trim_right_matches(".csv").to_string().replace(first_letter, &first_letter.to_string().to_uppercase())
    }
}

#[derive(Debug)]
struct DBResults {
    name: String,
    file_name: String,
    num_parsed: usize,
    num_stored: usize,
}

impl DBResults {
    pub fn new(name: String, file_name: String, num_parsed: usize, num_stored: usize) -> DBResults {
        DBResults{
            name,
            file_name,
            num_parsed,
            num_stored,
        }
    }

    pub fn get_results(&self, using_single_table: bool) -> Result<String, failure::Error> {
        let mut name = &self.name;
        if using_single_table {
            name = &self.file_name;
        }

        if self.num_stored != self.num_parsed {
           return  Err(failure::err_msg(format!("❌ {}: had {} errors", name, self.num_parsed - self.num_stored)));
        }

        Ok(format!("✅ {}: {} records loaded", name, &self.num_stored))
    }
}

#[derive(Debug, Clone)]
pub enum Types {
    MySQL,
    Postgres,
    SQLite,
}

impl FromStr for Types {
    type Err = error::DbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower_s: &str = &s.to_lowercase();
        match lower_s {
            "sqlite" => Ok(Types::SQLite),
            "postgres" => Ok(Types::Postgres),
            "mysql" => Ok(Types::MySQL),
            _ => Err(error::DbError::new(format!("ERROR: '{}' is not a supported database type", lower_s), exitcode::USAGE))
        }
    }
}

pub mod error {
    use failure::Fail;

    #[derive(Fail, Debug)]
    #[fail(display = "{}", msg)]
    pub struct DbError {
        msg: String,
        exit_code: exitcode::ExitCode,
    }

    impl DbError {
        pub fn get_exit_code(&self) -> exitcode::ExitCode {
            self.exit_code
        }

        pub fn get_msg(&self) -> String {
            self.msg.clone()
        }

        pub fn new(msg: String, exit_code: exitcode::ExitCode) -> DbError {
            DbError { msg, exit_code }
        }
    }
}

