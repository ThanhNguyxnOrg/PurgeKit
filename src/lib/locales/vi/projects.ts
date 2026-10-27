const projects: Record<string, string> = {
  // Header & actions
  "Universal Project Sweeper": "Dọn dự án toàn diện",
  "Locate and purge heavy build artifacts, temporary caches, and dependency folders in programming directories.": "Tìm và dọn các tệp build nặng, bộ nhớ đệm tạm và thư mục phụ thuộc trong các thư mục lập trình.",
  "Scanning...": "Đang quét...",
  "Start Sweep Scan": "Bắt đầu quét dọn",

  // Scan roots
  "Configured Scan Roots": "Thư mục quét gốc đã cấu hình",
  "Add folders that contain your active programming projects.": "Thêm thư mục chứa các dự án lập trình đang hoạt động.",
  "No scan paths configured. Add one below.": "Chưa cấu hình đường dẫn quét. Hãy thêm bên dưới.",
  "Remove Path": "Xoá đường dẫn",
  "e.g. D:\\Projects or D:\\Code": "Ví dụ: D:\\Projects hoặc D:\\Code",
  "Add directory path": "Thêm đường dẫn thư mục",

  // Target filters
  "Target Directory Filters": "Bộ lọc thư mục mục tiêu",
  "Select directory types to search for and purge.": "Chọn loại thư mục cần tìm và dọn.",
  "NodeJS Dependencies (node_modules)": "Phụ thuộc NodeJS (node_modules)",
  "Rust Build Artifacts (target)": "Tệp build Rust (target)",
  "Python Virtual Env (venv)": "Môi trường ảo Python (venv)",
  "Python Virtual Env (.venv)": "Môi trường ảo Python (.venv)",
  "VS Cache & Output (.vs)": "Bộ nhớ đệm & đầu ra VS (.vs)",
  "C# / .NET Binaries (bin)": "Tệp nhị phân C# / .NET (bin)",
  "C# / .NET Objects (obj)": "Tệp đối tượng C# / .NET (obj)",
  "Java/Gradle Build Output (build)": "Đầu ra build Java/Gradle (build)",
  "Gradle Cache (.gradle)": "Bộ nhớ đệm Gradle (.gradle)",
  "SvelteKit Cache (.svelte-kit)": "Bộ nhớ đệm SvelteKit (.svelte-kit)",
  "Next.js Build Files (.next)": "Tệp build Next.js (.next)",

  // Scan progress
  "Active Scan Traversal": "Đang duyệt quét",
  "Found:": "Đã tìm thấy:",
  "directories": "thư mục",
  "Searching: {path}": "Đang tìm: {path}",
  "Initializing...": "Đang khởi tạo...",
  "Scan complete! Found {count} project folders.": "Quét xong! Tìm thấy {count} thư mục dự án.",
  "Scan failed: {error}": "Quét thất bại: {error}",

  // Results summary
  "No Project Directories Loaded": "Chưa tải thư mục dự án nào",
  "Configure root paths and target filters, then run a Sweep Scan above.": "Cấu hình đường dẫn gốc và bộ lọc mục tiêu, sau đó chạy quét ở trên.",
  "Reclaimable Space": "Dung lượng có thể thu hồi",
  "Total Folders Found": "Tổng số thư mục tìm thấy",
  "{count} Directories": "{count} thư mục",
  "Selected for Purging": "Đã chọn để dọn",
  "{count} items ({size})": "{count} mục ({size})",

  // Table
  "Select All": "Chọn tất cả",
  "Deselect All": "Bỏ chọn tất cả",
  "{selected} of {total} selected": "Đã chọn {selected}/{total}",
  "Select": "Chọn",
  "Project Name": "Tên dự án",
  "Clean Target": "Mục tiêu dọn",
  "Size / Count": "Dung lượng / Số lượng",
  "Last Modified": "Sửa đổi lần cuối",
  "Directory Path": "Đường dẫn thư mục",
  "{count} files": "{count} tệp",
  "Unknown": "Không rõ",

  // Purge action bar
  "Sweep Plan Configured": "Kế hoạch quét dọn đã thiết lập",
  "Will delete": "Sẽ xoá",
  "project directories and free up": "thư mục dự án và giải phóng",
  "Cancel": "Huỷ",
  "Purging...": "Đang dọn...",
  "Purge Selected": "Dọn mục đã chọn",
  "Purging Project Directories": "Đang dọn thư mục dự án",
  "Deleting {current} / {total} items": "Đang xoá {current} / {total} mục",
  "Starting delete process...": "Đang bắt đầu quá trình xoá...",
  "No items selected for cleaning.": "Chưa chọn mục nào để dọn.",
  "Are you sure you want to permanently delete the {count} selected build/dependency directories?": "Bạn có chắc muốn xoá vĩnh viễn {count} thư mục build/phụ thuộc đã chọn?",
  "This will clear compiled binaries and packages, which can be re-installed/re-built later.": "Thao tác này sẽ xoá tệp nhị phân và gói đã biên dịch; bạn có thể cài lại/biên dịch lại sau.",
  "Project directories cleanup finished!": "Đã hoàn tất dọn thư mục dự án!",
  "Purge failed: {error}": "Dọn thất bại: {error}",

  // Deletion report
  "Sweeper Deletion Report": "Báo cáo xoá của trình dọn",
  "Review the final deletion results of selected directories.": "Xem kết quả xoá cuối cùng của các thư mục đã chọn.",
  "Boot-Pending": "Chờ khởi động lại",
  "Failed": "Thất bại",
  "Close Report": "Đóng báo cáo",

  // Validation & settings toasts
  "Error saving paths: {error}": "Lỗi khi lưu đường dẫn: {error}",
  "Please enter a valid directory path.": "Vui lòng nhập đường dẫn thư mục hợp lệ.",
  "Scanning drive roots directly is blocked for safety. Please select a specific folder inside the drive.": "Quét trực tiếp ổ đĩa gốc bị chặn để đảm bảo an toàn. Vui lòng chọn một thư mục cụ thể trong ổ.",
  "Protected system directory cannot be swept directly for security reasons.": "Không thể quét trực tiếp thư mục hệ thống được bảo vệ vì lý do an toàn.",
  "Protected system or program directory cannot be swept directly for security reasons.": "Không thể quét trực tiếp thư mục hệ thống hoặc thư mục chương trình được bảo vệ vì lý do an toàn.",
  "Directory path does not exist on your computer.": "Đường dẫn thư mục không tồn tại trên máy của bạn.",
  "This path is already configured.": "Đường dẫn này đã được cấu hình.",
  "Scan root path added successfully!": "Đã thêm đường dẫn quét gốc!",
  "Failed to validate path: {error}": "Không kiểm tra được đường dẫn: {error}",
  "Scan root path removed.": "Đã xoá đường dẫn quét gốc.",
  "Please add at least one scan root directory first.": "Vui lòng thêm ít nhất một thư mục quét gốc trước.",
  "Please check at least one target folder type to sweep.": "Vui lòng chọn ít nhất một loại thư mục mục tiêu để quét.",
};

export default projects;
