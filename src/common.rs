use std::time::*;
use crate::chat;

pub fn now_milli_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
}

pub fn human_milli_seconds(millis: u64) -> String {
     // 使用标准库将毫秒数转换为 SystemTime
    let duration = Duration::from_millis(millis);
    let system_time = UNIX_EPOCH + duration;
    
    // 将 SystemTime 转换为 DateTime<Utc>
    let datetime: chrono::DateTime<chrono::Utc> = system_time.into();
    
    // 使用 chrono 格式化时间为字符串
    let datetime_str = datetime.to_rfc2822(); 
    datetime_str
}

pub fn client_equal(c1: &chat::Client, c2: &chat::Client) -> bool{
    let thisname = c1.user.as_ref().unwrap().name.as_ref().unwrap();
    let othername = c2.user.as_ref().unwrap().name.as_ref().unwrap();
    if thisname == othername {
        return true;
    }
    return false;
}

pub fn client_in_room(client: &chat::Client, room: &chat::Room) -> bool {
    for c in room.clients.iter() {
        // 目前仅通过username来判断client是否在room里
        if client_equal(c, client) {
            return true;
        }
    }
    return false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_test() {
        let now = now_milli_seconds();
        println!("{now}");
    }

    #[test]
    fn cur_dir() {
        println!("{}", std::env::current_dir().unwrap().to_str().unwrap());
    }

    #[test]
    fn str_equal() {
        let a: String = String::from("abc");
        let b: String = String::from("abc");
        assert_eq!(a, b);
        assert_eq!(&a, &b);
    }
}
