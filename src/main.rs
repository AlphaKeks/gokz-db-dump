#![allow(non_snake_case)]

use {
	color_eyre::{
		eyre::{bail as yeet, Context},
		Report, Result,
	},
	gokz_rs::{Mode, SteamID},
	serde::Serialize,
	sqlx::{
		types::chrono::{NaiveDateTime, Utc},
		Connection, FromRow, SqliteConnection,
	},
	std::{fs::File, io::Write},
};

#[tokio::main]
async fn main() -> Result<()> {
	let args = std::env::args();
	let db_path = args
		.into_iter()
		.nth(1)
		.unwrap_or_else(|| String::from("./gokz-sqlite.sq3"));

	println!("Connecting to `{db_path}`...");

	let mut conn = SqliteConnection::connect(&db_path)
		.await
		.context("Failed to connect to database. Did you specify the file?")?;

	println!("Connected!");
	println!("Extracting records...");

	let records = sqlx::query_as::<_, RawRecord>(
		r#"
		SELECT * FROM Times
		"#,
	)
	.fetch_all(&mut conn)
	.await
	.context("Failed to get data.")?
	.into_iter()
	.filter_map(|row| match Record::try_from(row) {
		Ok(record) => Some(record),
		Err(why) => {
			eprintln!("Failed to parse record: {why:?}");
			None
		}
	})
	.collect::<Vec<_>>();

	println!("Successfully parsed records.");

	let now = Utc::now().format("%Y-%m-%d_%H-%M-%S");
	let file_name = format!("gokz-dump-{now}.json");

	let mut out_file = File::create(&file_name).context("Failed to create dump file.")?;

	let json = serde_json::to_vec_pretty(&records).context("Failed to parse records as JSON.")?;
	out_file
		.write_all(&json)
		.context("Failed to write JSON to disk.")?;

	println!("Wrote {} bytes to `{}`.", json.len(), file_name);

	Ok(())
}

#[allow(dead_code)]
#[derive(Debug, FromRow)]
struct RawRecord {
	TimeID: i32,
	SteamID32: i32,
	MapCourseID: i32,
	Mode: i32,
	Style: i32,
	RunTime: i32,
	Teleports: i32,
	Created: String,
}

#[derive(Debug, Clone, Serialize)]
struct Record {
	id: u32,
	steam_id: SteamID,
	map_id: u16,
	mode: Mode,
	time: f64,
	teleports: u32,
	created_at: String,
}

impl TryFrom<RawRecord> for Record {
	type Error = Report;

	fn try_from(row: RawRecord) -> Result<Self> {
		Ok(Self {
			id: u32::try_from(row.TimeID)?,
			steam_id: SteamID::from_id32(u32::try_from(row.SteamID32)?),
			map_id: u16::try_from(row.MapCourseID)?,
			mode: match row.Mode {
				0 => Mode::KZTimer,
				1 => Mode::SimpleKZ,
				2 => Mode::Vanilla,
				n => yeet!("{n} is not a valid mode"),
			},
			time: row.RunTime as f64 / 128.0,
			teleports: u32::try_from(row.Teleports)?,
			created_at: NaiveDateTime::parse_from_str(&row.Created, "%Y-%m-%d %H:%M:%S")
				.context("Failed to convert date")?
				.to_string(),
		})
	}
}
