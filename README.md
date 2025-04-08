# bin2hpp

> _One day we'll get #embed and std::embed, but today is not that day._

CLI tool for converting files into header files which one can use to directly embed data in their C++ projects.

## Building

1. `cargo build`

## Future improvements

- Not panicking if the source data is not UTF-8 encoded text when operating in text mode
- C support
- Choices between std::array, C-style arrays, std::string_view & C strings
- Customisable data types & data widths (unsigned vs. signed, uint8_t vs uint16_t, etc.)
