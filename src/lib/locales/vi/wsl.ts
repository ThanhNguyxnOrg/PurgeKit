const wsl: Record<string, string> = {
  // Header & stats
  "WSL2 Disk Shrinker": "Thu gọn ổ WSL2",
  "Reclaim physical hard disk space from bloated WSL2 virtual drive files (`ext4.vhdx`).": "Giải phóng dung lượng ổ cứng thật từ các tệp ổ đĩa ảo WSL2 phình to (`ext4.vhdx`).",
  "Refresh Distros": "Làm mới bản phân phối",
  "Total VHDX Disk Size": "Tổng dung lượng ổ VHDX",
  "WSL Distributions": "Bản phân phối WSL",
  "{count} Detected": "Phát hiện {count}",
  "Privileges": "Quyền hạn",
  "Privileged Mode Active": "Đang chạy với quyền quản trị",
  "Privileges Restricted": "Quyền hạn bị hạn chế",
  "Standard User (Run as Admin to Compact)": "Người dùng thường (chạy với quyền quản trị để thu gọn)",

  // Info box
  "Why do WSL2 virtual drives expand?": "Vì sao ổ ảo WSL2 phình to?",
  "WSL2 virtual drives (`ext4.vhdx`) grow as you write files inside Linux. However, when you delete files in Linux, the Windows host does not automatically shrink the `.vhdx` file. This utility allows you to manually co-root (compact) the virtual drives to reclaim unused storage or set them to sparse mode so Windows can automatically free up blocks in the background.": "Ổ đĩa ảo WSL2 (`ext4.vhdx`) tăng dung lượng khi bạn ghi tệp trong Linux. Tuy nhiên, khi bạn xoá tệp trong Linux, máy Windows không tự động thu nhỏ tệp `.vhdx`. Công cụ này cho phép bạn thu gọn ổ đĩa ảo để thu hồi dung lượng chưa dùng, hoặc bật chế độ sparse để Windows tự động giải phóng các block trong nền.",

  // Distribution list
  "WSL2 Distributions": "Bản phân phối WSL2",
  "Analyzing Windows Subsystem for Linux...": "Đang phân tích Windows Subsystem for Linux...",
  "No WSL2 distributions registered.": "Chưa có bản phân phối WSL2 nào.",
  "We could not find any active WSL configurations under HKCU registry.": "Không tìm thấy cấu hình WSL nào đang hoạt động trong registry HKCU.",
  "Drive size": "Dung lượng ổ",
  "VHDX Virtual Disk Path": "Đường dẫn ổ đĩa ảo VHDX",
  "Unable to locate ext4.vhdx disk file.": "Không tìm thấy tệp ổ đĩa ext4.vhdx.",
  "Disable Sparse Mode": "Tắt chế độ sparse",
  "Enable Sparse Mode": "Bật chế độ sparse",
  "Auto-Shrink": "Tự động thu gọn",
  "Sparse VHDX": "VHDX sparse",
  "Requires Administrator privileges": "Cần quyền quản trị viên",
  "Compact disk now": "Thu gọn ổ đĩa ngay",
  "Compact Disk": "Thu gọn ổ đĩa",

  // Compaction modal
  "VHDX Compaction Sequence:": "Tiến trình thu gọn VHDX:",
  "Status:": "Trạng thái:",
  "Shutting down WSL...": "Đang tắt WSL...",
  "Compacting Virtual Disk...": "Đang thu gọn ổ đĩa ảo...",
  "Completed": "Hoàn tất",
  "Failed": "Thất bại",
  "Processing virtual hard disk compaction...": "Đang xử lý thu gọn ổ đĩa ảo...",
  "Close Terminal": "Đóng terminal",
  "Compacting Disk...": "Đang thu gọn ổ đĩa...",

  // Toasts & confirmations
  "Failed to load WSL distros: {error}": "Không tải được bản phân phối WSL: {error}",
  "Enable Auto-Shrink (Sparse)": "Bật tự động thu gọn (Sparse)",
  "Disable Auto-Shrink (Thick)": "Tắt tự động thu gọn (Thick)",
  "{action} for {name}...": "{action} cho {name}...",
  "Successfully updated sparse property for {name}!": "Đã cập nhật thuộc tính sparse cho {name}!",
  "Failed to update sparse mode: {error}": "Không cập nhật được chế độ sparse: {error}",
  "WSL VHDX compaction requires Administrator privileges. Please run PurgeKit as Administrator.": "Thu gọn VHDX của WSL cần quyền quản trị viên. Vui lòng chạy PurgeKit với quyền quản trị viên.",
  "Could not locate the VHDX file path for this distribution.": "Không tìm thấy đường dẫn tệp VHDX của bản phân phối này.",
  "Are you sure you want to compact the virtual disk for {name}?": "Bạn có chắc muốn thu gọn ổ đĩa ảo của {name}?",
  "This will temporarily shutdown WSL and use the Windows diskpart utility to reclaim deleted storage.": "Thao tác này sẽ tạm tắt WSL và dùng tiện ích diskpart của Windows để thu hồi dung lượng đã xoá.",
  "Successfully compacted virtual disk for {name}!": "Đã thu gọn ổ đĩa ảo của {name}!",
  "Compaction failed: {error}": "Thu gọn thất bại: {error}",
};

export default wsl;
