mod configuration;

use configuration::{
    get_configuration_file_path,
    load_configuration,
    print_libraries,
};


fn main() {
    let configuration_filepath = get_configuration_file_path();
    println!("Configuration file: {:?}.", configuration_filepath);

    let config = load_configuration(configuration_filepath);
    println!("Loaded configuration, {} libraries available.", config.libraries.len());

    print_libraries(config.libraries);
}
