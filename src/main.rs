extern crate csv_to;
extern crate exitcode;
extern crate structopt;

use csv_to::CsvTo;
use csv_to::db::DbApp;
use csv_to::db::config::Config;
use csv_to::adapters::csvinput::CSVService;
use csv_to::storage::sqlite::SQLiteStore;
use structopt::StructOpt;

fn main() {
    let opt = CsvTo::from_args();

    // As I build out the sub-commands this match will have multiple options, all of which will
    // implement the App trait
    let app = match opt {
        CsvTo::Db { files, directories, db_type, connection_info, name, drop_stores, no_headers } => {
            if files.is_empty() && directories.is_empty() {
                eprintln!("error: either -f, --files or -d, --directories must be provided");
                std::process::exit(exitcode::USAGE);
            }

            DbApp::new(
                Config::new(files, directories, db_type, connection_info.clone(), name, drop_stores, no_headers),
                CSVService::default(),
                SQLiteStore::new(connection_info.clone()).unwrap_or_else(|err| {
                    eprintln!("error while attempting to create a database connection: {}", err);
                    std::process::exit(exitcode::USAGE);
                }),
            )
        }
    };

    // This is where the logic is kicked off
    app.run().unwrap_or_else(|err| {
        eprintln!("ERROR: {}", err);
        std::process::exit(exitcode::IOERR);
    });
}

