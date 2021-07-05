script;
// this file tests struct subfield reassignments
fn main() {
  let mut data = Data { 
                  value: NumberOrString::Number(20),
                  address: 0b00001111,
                };

  data.value = NumberOrString::String("sway");
}


enum NumberOrString {
  Number: u64,
  String: str[4],
}

struct Data {
  value: NumberOrString,
  address: byte,
}
