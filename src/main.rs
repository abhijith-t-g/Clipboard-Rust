mod history;
use history::ClipboardHistory;

fn main() {
    let mut history = ClipboardHistory::new(5);
    history.add("Hello world! 1".to_string());
    history.add("Hello world! 2".to_string());

    let list = history.get_items();
    for i in list.iter() {
        println!("{:#?}, {:#?}", i.content, i.timestamp);
    }
}
