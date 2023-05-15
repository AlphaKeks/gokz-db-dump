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
	std::fs::File,
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
		SELECT
		  t.*,
		  m.MapID AS MapID,
		  m.Name AS MapName,
		  c.Course AS Course,
		  p.Alias AS PlayerName
		FROM Times AS t
		JOIN MapCourses AS c
		ON c.MapCourseID = t.MapCourseID
		JOIN Maps AS m
		ON m.MapID = c.MapID
		JOIN Players AS p
		ON p.SteamID32 = t.SteamID32
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
	let file_name = format!("gokz-dump-{now}.csv");

	let out_file = File::create(&file_name).context("Failed to create dump file.")?;
	let mut csv_writer = csv::Writer::from_writer(out_file);

	let n_records = records.len();

	for record in records {
		if let Err(why) = csv_writer.serialize(record) {
			eprintln!("Failed to serialize record as CSV: {why:?}");
		}
	}

	println!("Wrote {} records to `{}`.", n_records, file_name);

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
	MapName: String,
	MapID: i32,
	Course: i32,
	PlayerName: String,
}

#[derive(Debug, Clone, Serialize)]
struct Record {
	id: u32,
	steam_id: SteamID,
	player_name: String,
	map_id: u16,
	map_name: String,
	stage: u8,
	mode: Mode,
	time: f64,
	teleports: u32,
	created_on: String,
}

impl TryFrom<RawRecord> for Record {
	type Error = Report;

	fn try_from(row: RawRecord) -> Result<Self> {
		Ok(Self {
			id: u32::try_from(row.TimeID)?,
			steam_id: SteamID::from_id32(u32::try_from(row.SteamID32)?),
			player_name: row.PlayerName,
			map_id: u16::try_from(row.MapID)?,
			map_name: row.MapName,
			stage: u8::try_from(row.Course)?,
			mode: match row.Mode {
				0 => Mode::Vanilla,
				1 => Mode::SimpleKZ,
				2 => Mode::KZTimer,
				n => yeet!("{n} is not a valid mode"),
			},
			time: row.RunTime as f64 / 1000.0,
			teleports: u32::try_from(row.Teleports)?,
			created_on: NaiveDateTime::parse_from_str(&row.Created, "%Y-%m-%d %H:%M:%S")
				.context("Failed to convert date")?
				.to_string(),
		})
	}
}
