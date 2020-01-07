use failure::{format_err, Error};
use std;
use std::ffi::{CStr, FromBytesWithNulError};
use std::mem;
use bytes::Buf;

#[derive(Debug)]
pub struct Command {
    header: RecordHeader,
}

impl Command {
    pub fn search() -> Command {
        Command {
            header: RecordHeader {
                device_name: "PC2000".into(),
                command: CommandType::Search,
                argument: ArgumentType::None,
            },
        }
    }

    pub fn query() -> Command {
        Command::read(ArgumentType::NowRecord)
    }

    fn read(argument: ArgumentType) -> Command {
        Command {
            header: RecordHeader {
                device_name: "PC2000".into(),
                command: CommandType::Read,
                argument,
            },
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let result = CSearchRequest {
            header: self.header.to_c_record()?,
            _unknown : [0u8; 8]
        };

        Ok(unsafe { Command::any_as_u8_slice(&result) })
    }

    unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> Vec<u8> {
        ::std::slice::from_raw_parts(
            (p as *const T) as *const u8,
            ::std::mem::size_of::<T>(),
        ).to_vec()
    }
}

/// READ appears to specify a "request" vs. the response which is WRITE. If I were to guess, this
/// allows a single struct to be reused for both and why nul-padding the READ doesn't actually
/// do anything.
#[derive(Debug)]
pub enum CommandType {
    Unknown,
    Read,
    Search,
    Write,
}

impl CommandType {
    fn from_str(command: &str) -> CommandType {
        match command {
            "READ" => CommandType::Read,
            "SEARCH" => CommandType::Search,
            "WRITE" => CommandType::Write,
            _ => CommandType::Unknown,
        }
    }

    fn to_string(&self) -> &str {
        match *self {
            CommandType::Unknown => "UNKNOWN",
            CommandType::Read => "READ",
            CommandType::Search => "SEARCH",
            CommandType::Write => "WRITE",
        }
    }
}

#[derive(Debug)]
pub enum ArgumentType {
    Unknown,
    None,
    Query,
    Search,
    NowRecord,
    HistoryData,
}

impl ArgumentType {
    fn from_str(argument: &str) -> ArgumentType {
        match argument {
            "QUERY" => ArgumentType::Query,
            "NOWRECORD" => ArgumentType::NowRecord,
            "HISTORY_DATA" => ArgumentType::HistoryData,
            _ => ArgumentType::Unknown,
        }
    }
    fn to_string(&self) -> &str {
        match *self {
            ArgumentType::Unknown => "UNKNOWN",
            ArgumentType::None => "",
            ArgumentType::Query => "QUERY",
            ArgumentType::Search => "SEARCH",
            ArgumentType::NowRecord => "NOWRECORD",
            ArgumentType::HistoryData => "HISTORY_DATA",
        }
    }
}

#[derive(Debug)]
pub struct RecordHeader {
    pub device_name: String,
    pub command: CommandType,
    pub argument: ArgumentType,
}

impl RecordHeader {
    fn from_c_record(header: &CRecordHeader) -> Result<RecordHeader, Error> {
        let header = RecordHeader {
            device_name: from_bytes_nul_padded(&header.device_name)?.to_str()?.into(),
            command: CommandType::from_str(from_bytes_nul_padded(&header.command)?.to_str()?),
            argument: ArgumentType::from_str(from_bytes_nul_padded(&header.argument)?.to_str()?),
        };

        Ok(header)
    }

    fn to_c_record(&self) -> Result<CRecordHeader, Error> {
        let mut device_name: [u8; 8] = Default::default();
        let mut command: [u8; 8] = Default::default();
        let mut argument: [u8; 16] = Default::default();

        device_name.copy_from_slice(&to_bytes_nul_padded(self.device_name.as_str(), 8)?);
        command.copy_from_slice(&to_bytes_nul_padded(self.command.to_string(), 8)?);
        argument.copy_from_slice(&to_bytes_nul_padded(self.argument.to_string(), 16)?);

        let c_header = CRecordHeader {
            device_name,
            command,
            argument,
        };

        Ok(c_header)
    }
}

#[derive(Debug)]
pub enum Response {
    WeatherRecord(WeatherRecord)
}

impl Response {
    pub fn from_bytes(bytes: &[u8]) -> Result<Response, Error> {
        let c_record_header = unsafe { CRecordHeader::from_bytes(bytes) };
        let header = RecordHeader::from_c_record(&c_record_header)?;

        match header.argument {
            ArgumentType::NowRecord => {
                Ok(Response::WeatherRecord(WeatherRecord::parse(bytes)?))
            },
            _ => todo!(),
        }
    }
}

#[derive(Debug)]
pub struct WeatherRecord {
    record_header: RecordHeader,
    wind: WindRecord,
    inside: TemperatureAndHumidity,
    outside: TemperatureAndHumidity,
    pressure: f32,
    barometer: f32,
    dewpoint: f32,
    rain: RainRecord,
    radiation: f32,
    uv_index: u8,
    heat_index: u8,
}

impl WeatherRecord {
    pub fn parse(bytes: &[u8]) -> Result<WeatherRecord, Error> {
        let c_record = unsafe { CNowRecord::from_bytes(bytes) };

        let record = WeatherRecord {
            record_header: RecordHeader::from_c_record(&c_record.record_header)?,
            wind: WindRecord {
                direction: c_record.wind_direction,
                wind_chill: c_record.wind_chill,
                wind_speed: c_record.wind_speed,
                wind_gust: c_record.wind_gust,
            },
            inside: TemperatureAndHumidity {
                temperature: c_record.inside_temperature,
                humidity_percent: c_record.inside_humidity,
            },
            outside: TemperatureAndHumidity {
                temperature: c_record.outside_temperature,
                humidity_percent: c_record.outside_humidity,
            },
            pressure: c_record.pressure,
            barometer: c_record.barometer,
            dewpoint: c_record.dewpoint,
            rain: RainRecord {
                rain_rate: c_record.rain_rate,
                daily_rain: c_record.daily_rain,
                weekly_rain: c_record.weekly_rain,
                yearly_rain: c_record.yearly_rain,
            },
            radiation: c_record.radiation,
            uv_index: c_record.uv_index,
            heat_index: c_record.heat_index,
        };

        Ok(record)
    }
}

#[derive(Debug)]
pub struct WindRecord {
    pub direction: i16,
    pub wind_chill: f32,
    pub wind_speed: f32,
    pub wind_gust: f32,
}

#[derive(Debug)]
pub struct RainRecord {
    pub rain_rate: f32,
    pub daily_rain: f32,
    pub weekly_rain: f32,
    pub yearly_rain: f32,
}

#[derive(Debug)]
pub struct TemperatureAndHumidity {
    pub temperature: f32,
    pub humidity_percent: u8,
}

#[derive(Debug)]
pub struct SearchResponse {
    name: String,
    command: String,
    //    mac_address: String,
    ip_address: String,
}

/// These are structures used in decoding responses from the WS-1001 Observer unit. These are C
/// structs which can be trivially decoded by transmuting the memory.

// Offset  Value           Structure       Comment
// 0x00    HP2000          8 byte string   Name of the weather station
// 0x08    SEARCH          8 byte string   Command
// 0x10                    8 byte string   Argument
// Note: strings are NUL-terminated + NUL-padded.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct CRecordHeader {
    device_name: [u8; 8],
    command: [u8; 8],
    argument: [u8; 16],
}

impl CRecordHeader {
    /// We ignore cast_ptr_alignment as we use std::ptr::read_unaligned.
    #[allow(clippy::cast_ptr_alignment)]
    unsafe fn from_bytes(bytes: &[u8]) -> CRecordHeader {
        assert!(bytes.len() >= mem::size_of::<CRecordHeader>());

        std::ptr::read_unaligned(bytes[0..32].as_ptr() as *const CRecordHeader)
    }
}

// Offset  Value           Structure       Comment
// 0x00    HP2000          8 byte string   Name of the weather station
// 0x08    SEARCH          8 byte string   Command
// 0x10                    8 byte string   Argument
// 0x18                    16 bytes        Not yet deciphered
// 0x28    text            24 byte string  MAC address of the weather station
// 0x40    text            16 byte string  IP address of the weather station
#[derive(Debug)]
#[repr(C)]
pub struct CSearchRequest {
    header: CRecordHeader,
    _unknown: [u8; 8],
}

// Offset  Value           Structure       Comment
// 0x00    HP2000          8 byte string   Name of the weather station
// 0x08    SEARCH          8 byte string   Command
// 0x10                    8 byte string   Argument
// 0x18                    16 bytes        Not yet deciphered
// 0x28    text            24 byte string  MAC address of the weather station
// 0x40    text            16 byte string  IP address of the weather station
#[derive(Debug)]
#[repr(C)]
pub struct CSearchResponse {
    // This may be irrelevant.
}

fn from_bytes_nul_padded(bytes: &[u8]) -> Result<&CStr, FromBytesWithNulError> {
    let nul_index = bytes.iter().position(|&b| b == b'\0').unwrap_or(0);

    CStr::from_bytes_with_nul(&bytes[..=nul_index])
}

fn to_bytes_nul_padded(string: &str, length: usize) -> Result<Vec<u8>, Error> {
    if string.len() >= length {
        return Err(format_err!("String '{}' >= max length of {}.", string, length))
    }

    let bytes_to_pad = length - string.len();
    let mut bytes = string.to_string();

    (0 .. bytes_to_pad)
        .for_each(|_| bytes += "\0");

    Ok(bytes.into_bytes())
}

// Offset  Value       Structure   Comment
// 0x20    unknown     16 bytes    Yet to be deciphered
// 0x30    Time        1 byte      1 = 'H:mm:ss', 2="h:mm:ss AM", 4='AM h:mm:ss'
// 0x31    Date        1 byte      16 = 'DD-MM-YYYY', 32 = 'MM-DD-YYYY', 64 = 'YYYY-MM-DD'
// 0x32    Temperature 1 byte      0 = Celsius, 1 = Fahrenheit
// 0x33    Pressure    1 byte      0 = hPa, 1 = inHg, 2 = mmHg
// 0x34    Wind speed  1 byte      0 = m/s, 1 = km/h, 2 = knots, 3 = mph, 4 = Beaufort, 5 = ft/s
// 0x35    Rainfall    1 byte      0 = mm, 1 = in
// 0x36    Solar rad   1 byte      0 = lux, 1 = fc, 2=W/m^2
// 0x37    Rain display1 byte      0 = rain rate, 1 = daily, 2 = weekly, 3 = monthly, 4 = yearly
// 0x38    Graph time  1 byte      0 = 12h, 1 = 24h, 2 = 48h, 3 = 72h
// 0x39    Barometer   1 byte      0 = absolute, 1 = relative
// 0x3a    Weather     1 byte      number
// 0x3b    Storm       1 byte      number
// 0x3c    Current     1 byte      0 = sunny, 1 = partly cloudy, 2 = cloudy, 3 = raim. 4 = strom
// 0x3d    Reset       1 byte      Month for yearly rain reset, 1 = Jan, 2 = Feb...
// 0x3e    Update      1 byte      Update interval in minutes
#[derive(Debug)]
#[repr(C)]
struct CSetupResponse {}

// Offset  Value       Structure   Type   Comment
// 0x20    unknown     8 bytes     u64    Timestamp in microseconds since 1/1/1601(!)
// 0x28    Wind dir    2 bytes     u32    Wind direction (in degrees from North = 0) [4]
// 0x2a    inHumidity  1 byte      u8     Inside humidity [5] (percent)
// 0x2b    outHumidity 1 byte      u8     Outside humidity [6] (percent)
// 0x2c    inTemp      4 bytes     f64    Inside temperature (floating point) [7]
// 0x30    pressure    4 bytes     f64    Relative pressure (floating point) [8]
// 0x34    barometer   4 bytes     f64    Absolute pressure (floating point) [9]
// 0x38    outTemp     4 bytes     f64    Outside temperature (floating point) [10]
// 0x3c    dewPoint    4 bytes     f64    Dew Point tempperature (floating point) [11]
// 0x40    windChill   4 bytes     f64    Wind Chill temperature (floating point) [12]
// 0x44    windSpeed   4 bytes     f64    Wind speed (floating point) [13]
// 0x48    windGust    4 bytes     f64    Wind Gust (floating point) [14]
// 0x4c    rainRate    4 bytes     f64    Rain rate (floating point) [15]
// 0x50    dailyRain   4 bytes     f64    Daily rain (floating point) [16]
// 0x54    weeklyRain  4 bytes     f64    Weekly rain (floating point) [17]
// 0x58    monthlyRain 4 bytes     f64    Monthly rain (floating point) [18]
// 0x5c    yearlyRain  4 bytes     f64    Yearly rain (floating point) [19]
// 0x60    radiation   4 bytes     f64    Current solar radiation(floating point) [20]
// 0x64    UVI         1 byte      u8     UV Index [21]
// 0x65    heatIndex    1 bytes     u8    Heat Index [22] luminosity lux
// 0x66                2 bytes     ???    Unknown [23]
//
// Unpack NOWRECORD message received from console
//(@msgcontent) = unpack("A8 A8 Z16 S C I C S C2 f14 C2", $rcvmsg);

#[derive(Debug)]
#[repr(C)]
struct CNowRecord {
    record_header: CRecordHeader,
    _unknown: [u8; 8],
    wind_direction: i16,
    inside_humidity: u8,
    outside_humidity: u8,
    inside_temperature: f32,
    pressure: f32,
    barometer: f32,
    outside_temperature: f32,
    dewpoint: f32,
    wind_chill: f32,
    wind_speed: f32,
    wind_gust: f32,
    rain_rate: f32,
    daily_rain: f32,
    weekly_rain: f32,
    monthly_rain: f32,
    yearly_rain: f32,
    radiation: f32,
    uv_index: u8,
    heat_index: u8,
    _unknown3: u16,
}

impl CNowRecord {
    /// We ignore cast_ptr_alignment as we use std::ptr::read_unaligned.
    #[allow(clippy::cast_ptr_alignment)]
    unsafe fn from_bytes(bytes: &[u8]) -> CNowRecord {
        assert!(bytes.len() >= mem::size_of::<CNowRecord>());

        std::ptr::read_unaligned(bytes.as_ptr() as *const CNowRecord)
    }
}
