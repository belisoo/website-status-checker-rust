# website-status-checker-rust
To run the program and test out the 50 website URLs in websites.txt use command:
```
cargo run -- --file websites.txt --workers 10 --timeout 10 --retries 0
```

This command makes sure all 50 URLS are read and no URLs are cut off for taking too long.
Make sure to have an integrated terminal with the website_checker program before running the command.

This project's purpose was to design and implement a concurrent website‑monitoring tool that can check the availability of many websites in parallel. It checks if the website is a valid website/URL. I used the US's 50 top websites and a fake website to test out the code. We have to write the results into a JSON file. We use multi-threading to make checking as efficient as possible. 