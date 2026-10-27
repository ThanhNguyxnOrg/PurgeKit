const snapshots: Record<string, string> = {
  // Header
  "System Snapshots": "Ảnh chụp hệ thống",
  "Track system changes by comparing snapshots taken before and after app installations for 100% clean uninstall":
    "Theo dõi thay đổi hệ thống bằng cách so sánh ảnh chụp trước và sau khi cài đặt ứng dụng để gỡ cài đặt sạch 100%",
  "Install Monitor": "Giám sát cài đặt",
  "Create Snapshot": "Tạo ảnh chụp",
  "Refresh": "Làm mới",

  // Active tracking banner
  "Installation Monitor Active: Tracking changes for \"{name}\"... Please run your software installer now.":
    "Giám sát cài đặt đang hoạt động: Đang theo dõi thay đổi cho \"{name}\"... Vui lòng chạy trình cài đặt phần mềm ngay.",
  "Saving Snapshot...": "Đang lưu ảnh chụp...",
  "Stop & Save Snapshot": "Dừng và lưu ảnh chụp",

  // Notice banner
  "How Active Monitoring Works": "Cách giám sát hoạt động",
  "1. Take a snapshot": "1. Tạo ảnh chụp",
  "before": "trước khi",
  "installing a new application.": "cài đặt một ứng dụng mới.",
  "2. Install the application and run it once.": "2. Cài đặt ứng dụng và chạy nó một lần.",
  "3. Take a snapshot": "3. Tạo ảnh chụp",
  "after": "sau khi",
  "installation.": "cài đặt.",
  "PurgeKit compares them, tracking every registry entry and file created, guaranteeing 100% trace-free removal.":
    "PurgeKit sẽ so sánh chúng, theo dõi mọi mục registry và tệp được tạo, đảm bảo xoá sạch 100% không để lại dấu vết.",

  // Workflow stepper
  "Take Baseline": "Chụp mốc ban đầu",
  "Scan before install": "Quét trước khi cài đặt",
  "Install Software": "Cài đặt phần mềm",
  "Run app installer": "Chạy trình cài đặt ứng dụng",
  "Compare Changes": "So sánh thay đổi",
  "Generate clean diff": "Tạo bản so sánh sạch",

  // Snapshot list
  "Saved System Snapshots": "Ảnh chụp hệ thống đã lưu",
  "Loading saved snapshots...": "Đang tải ảnh chụp đã lưu...",
  "No Snapshots Found": "Không tìm thấy ảnh chụp nào",
  "Take a snapshot before installing apps to start tracking.":
    "Tạo ảnh chụp trước khi cài ứng dụng để bắt đầu theo dõi.",
  "Delete snapshot": "Xoá ảnh chụp",
  "Registry Keys": "Khoá Registry",
  "Files Tracked": "Tệp được theo dõi",
  "Set BEFORE": "Đặt làm TRƯỚC",
  "Set AFTER": "Đặt làm SAU",

  // Compare sidebar
  "Compare snapshots": "So sánh ảnh chụp",
  "Snapshot BEFORE installation": "Ảnh chụp TRƯỚC khi cài đặt",
  "Snapshot AFTER installation": "Ảnh chụp SAU khi cài đặt",
  "Select a snapshot below...": "Chọn ảnh chụp bên dưới...",
  "Reset": "Đặt lại",
  "Comparing...": "Đang so sánh...",
  "Compare": "So sánh",

  // Diff results
  "Comparison Results ({count} differences found)": "Kết quả so sánh ({count} khác biệt được tìm thấy)",
  "New Registry Keys ({count})": "Khoá Registry mới ({count})",
  "No new registry keys detected.": "Không phát hiện khoá Registry mới.",
  "New Registry Values ({count})": "Giá trị Registry mới ({count})",
  "No new registry values detected.": "Không phát hiện giá trị Registry mới.",
  "New Files & Folders ({count})": "Tệp & thư mục mới ({count})",
  "No new files detected.": "Không phát hiện tệp mới.",

  // Create snapshot modal
  "Create System Snapshot": "Tạo ảnh chụp hệ thống",
  "Take a scan of important registry directories & application folders to establish a before/after baseline.":
    "Quét các thư mục registry và thư mục ứng dụng quan trọng để thiết lập mốc so sánh trước/sau.",
  "Snapshot Name": "Tên ảnh chụp",
  "e.g., Before Docker Install": "vd: Trước khi cài Docker",
  "Cancel": "Huỷ",
  "Scanning System...": "Đang quét hệ thống...",
  "Scan Baseline": "Quét mốc ban đầu",

  // Install tracking modal
  "Real-time Install Monitor": "Giám sát cài đặt thời gian thực",
  "PurgeKit will record a baseline and monitor file creations (using the NTFS USN Journal) on every ready NTFS volume, plus registry entries, in real-time.":
    "PurgeKit sẽ ghi lại mốc ban đầu và theo dõi việc tạo tệp (dùng NTFS USN Journal) trên mọi ổ NTFS sẵn sàng, cùng các mục registry, trong thời gian thực.",
  "Application Name / Session ID": "Tên ứng dụng / ID phiên",
  "e.g., Python 3.12, VS Code": "vd: Python 3.12, VS Code",
  "Initializing...": "Đang khởi tạo...",
  "Start Tracking": "Bắt đầu giám sát",

  // Toasts / confirmations
  "Error loading snapshots list!": "Lỗi khi tải danh sách ảnh chụp!",
  "Please enter a name for the snapshot.": "Vui lòng nhập tên cho ảnh chụp.",
  "Snapshot \"{name}\" created successfully!": "Đã tạo ảnh chụp \"{name}\" thành công!",
  "Failed to create snapshot: {error}": "Tạo ảnh chụp thất bại: {error}",
  "Please enter a name for the installation session.": "Vui lòng nhập tên cho phiên cài đặt.",
  "Started installation tracking for \"{name}\"!": "Đã bắt đầu theo dõi cài đặt cho \"{name}\"!",
  "Failed to start tracking: {error}": "Bắt đầu theo dõi thất bại: {error}",
  "Finished installation tracking for \"{session}\"! Snapshot \"{snapshot}\" created.":
    "Đã kết thúc theo dõi cài đặt cho \"{session}\"! Đã tạo ảnh chụp \"{snapshot}\".",
  "Error stopping installation tracking: {error}": "Lỗi khi dừng theo dõi cài đặt: {error}",
  "Please select both BEFORE and AFTER snapshots to compare.":
    "Vui lòng chọn cả ảnh chụp TRƯỚC và SAU để so sánh.",
  "Please select two different snapshots to compare.":
    "Vui lòng chọn hai ảnh chụp khác nhau để so sánh.",
  "Snapshot comparison completed!": "Đã hoàn tất so sánh ảnh chụp!",
  "Comparison failed: {error}": "So sánh thất bại: {error}",
  "Are you sure you want to delete this snapshot?": "Bạn có chắc muốn xoá ảnh chụp này?",
  "Snapshot deleted successfully!": "Đã xoá ảnh chụp thành công!",
  "Failed to delete snapshot: {error}": "Xoá ảnh chụp thất bại: {error}",
};

export default snapshots;
