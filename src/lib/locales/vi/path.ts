const path: Record<string, string> = {
  // Header
  "PATH Environment Cleaner": "Dọn biến môi trường PATH",
  "Scan and clean broken, dead, or duplicate paths in your Windows PATH variables to avoid environment conflicts":
    "Quét và dọn các đường dẫn hỏng, không tồn tại hoặc trùng lặp trong biến PATH của Windows để tránh xung đột môi trường",
  "Clean Selected Paths": "Dọn các đường dẫn đã chọn",
  "Scan PATH": "Quét PATH",

  // Stats
  "Total Scanned": "Tổng đã quét",
  "{count} items": "{count} mục",
  "Issues Detected": "Vấn đề phát hiện",
  "{count} errors": "{count} lỗi",
  "Clean Entries": "Mục sạch",

  // List
  "Path Variables": "Biến đường dẫn",
  "Scanning Windows PATH directories...": "Đang quét các thư mục PATH của Windows...",
  "No PATH values found!": "Không tìm thấy giá trị PATH nào!",

  // Table
  "Select": "Chọn",
  "Variable Scope": "Phạm vi biến",
  "Path Value": "Giá trị đường dẫn",
  "Status": "Trạng thái",
  "Details / Reason": "Chi tiết / Lý do",
  "Broken": "Hỏng",
  "Duplicate": "Trùng lặp",
  "Valid": "Hợp lệ",
  "Directory does not exist": "Thư mục không tồn tại",
  "Duplicate of line #{line}": "Trùng với dòng #{line}",
  "System": "Hệ thống",
  "User": "Người dùng",

  // Toasts
  "Error loading PATH list!": "Lỗi khi tải danh sách PATH!",
  "Please select at least one path to clean.": "Vui lòng chọn ít nhất một đường dẫn để dọn.",
  "Modifying System PATH requires Administrator privileges. Please run PurgeKit as Administrator.":
    "Sửa PATH hệ thống cần quyền Quản trị viên. Vui lòng chạy PurgeKit với quyền Quản trị viên.",
  "Cannot clear all System PATH entries! Windows requires system PATH to function.":
    "Không thể xoá toàn bộ mục PATH hệ thống! Windows cần PATH hệ thống để hoạt động.",
  "Cleaned invalid/duplicate paths successfully!":
    "Đã dọn các đường dẫn không hợp lệ/trùng lặp thành công!",
  "Failed to clean: {error}": "Dọn thất bại: {error}",
};

export default path;
