use reqwest::{Client, ClientBuilder, Method};
use serde_json::json;
use serde_json::Value;
use std::error::Error;
use std::fmt::Display;

pub const ENDPOINT: &str = "/WebAPI.mvc/Mobile/SchoolToolMobile";
#[derive(Debug)]
pub struct SchoolTool {
    auth_header: String,
    client: Client,
    base_url: String,
    guid: String,
}
#[derive(Debug, Clone)]
pub struct Student {
    pub name: String,
    pub guid: String,
    pub cycle_day: Option<u8>,
}
#[derive(Debug)]
struct DataError {}
impl Error for DataError{}
impl Display for DataError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error parsing data.");
        Ok(())
    }
}

impl SchoolTool {
    pub async fn new(
        base_url: String,
        username: String,
        password: String,
    ) -> Result<Self, Box<dyn Error>> {
        let client = ClientBuilder::new().user_agent("Mozilla/5.0 (X11; CrOS x86_64 14695.142.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36")
        .build()
        .unwrap();

        let password_hash = encode_password(password);

        let login_struct = &log_in(&client, &username, &password_hash, &base_url).await?;
        let guid_blob = login_struct.get("PersonGuid").unwrap().as_str().unwrap();
        let auth_header = format!(
            "{} {}:{}",
            login_struct.get("Key").ok_or(DataError{})?.as_str().unwrap(),
            guid_blob,
            &password_hash
        );

        let guid = guid_blob.split("|").nth(1).ok_or(DataError{})?.to_string();
        Ok(Self {
            base_url,
            auth_header,
            client,
            guid,
        })
    }
    pub async fn get_student(&self, student_guid: Option<&str>) -> Result<Student, Box<dyn Error>> {
        let homereq = self
            .client
            .request(Method::POST, format!("{}{}/Home", self.base_url, ENDPOINT))
            .header("Content-Type", "application/json;charset=utf-8")
            .header("authorization", &self.auth_header)
            .body("\"\"");
        let body = homereq.send().await?.text().await?;
        let home_struct: Value = serde_json::from_str(&body)?;
        let student_struct = home_struct
            .get("Students")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|v| {
                v.get("StudentPersonGuid").unwrap().as_str().unwrap()
                    == student_guid.unwrap_or(&self.guid)
            })
            .unwrap();

        let cycle_day = student_struct
            .get("StudentBuildingSchoolLevelCycleDays")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .get("CycleDay")
            .unwrap()
            .as_str()
            .unwrap();
        let name = format!(
            "{} {} {}",
            student_struct.get("FirstName").unwrap().as_str().unwrap(),
            student_struct.get("MiddleName").unwrap().as_str().unwrap(),
            student_struct.get("LastName").unwrap().as_str().unwrap()
        );
        Ok(Student {
            guid: student_struct
                .get("StudentPersonGuid")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
            name,
            cycle_day: u8::from_str_radix(cycle_day, 10).ok(),
        })
    }
    pub async fn quarter_data(
        &self,
        data_type: String,
        guid: String,
        quarter: Value,
    ) -> Result<Value, Box<dyn Error>> {
        let g = self
            .client
            .request(
                Method::POST,
                format!("{}{}/{}", self.base_url, ENDPOINT, data_type),
            )
            .header("Content-Type", "application/json;charset=utf-8")
            .header("authorization", &self.auth_header)
            .body(
                json!({
                    "studentGuid":guid,
                    "buildingSchoolLevelId":4,
                    "markingPeriodId":quarter,
                    "asOfDate":null,
                })
                .to_string(),
            );

        let raw = g.send().await?.text().await?;
        Ok(serde_json::from_str(&raw)?)
    }
}
async fn log_in(
    client: &Client,
    username: &str,
    password_hash: &str,
    base_url: &str,
) -> Result<Value, Box<dyn Error>> {
    let req = client
        .request(Method::POST, format!("{}{}/AppLogin", base_url, ENDPOINT))
        .header("Content-Type", "application/json;charset=utf-8")
        .body(
            json!({
                "username": username,
                "password": password_hash
            })
            .to_string(),
        );
    Ok(serde_json::from_str(&req.send().await?.text().await?)?)
}
fn encode_password(password: String) -> String {
    let passlen = password.chars().count();

    let salt = "tAYOdhqzEERgIbU8WGdH2EI6YS77pILeLVsOjVd5gzVvX43Blm";
    let salt2 = "D12H";
    let mut buffer3 = String::new();

    for i in 0..passlen {
        buffer3 += &(password.chars().nth(passlen - (i + 1)).unwrap().to_string()
            + &salt.chars().nth(i).unwrap().to_string());
    }
    let mut buffer4 = String::new();
    for ch in buffer3.chars() {
        buffer4 += &format!("{:x}{}", ch as u8, salt2);
    }
    base64::encode(buffer4)
}

// this is never used. just for shits and giggles and a bit of code golfing.
fn _decode_password(encoded: String) -> String {
    let bytes = &base64::decode(encoded).unwrap();
    let s = std::str::from_utf8(bytes).unwrap();
    (0..s.len())
        .step_by(12)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap() as char)
        .rev()
        .collect()
}
