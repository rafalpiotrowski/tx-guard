# tx-guard
simple transaction verification client

output of the application is send to the std out

# main assumptions
input data file format: tx type, client id, tx id, tx amount
tx type: String
client id: u16 (max 65535)
tx id: u32 (max 4294967295)
tx amount: decimal value with precision of upto 4 places past the decimal

# Stream values through memory

# How to handle multiple data sources?

# How to run it?
$ cargo run -- input_data_file_name.csv > output_data_file_name.csv

# used dependencies
csv-async
