const apps: Record<string, string> = {
  // Header & toolbar
  "Apps Manager": "Quản lý ứng dụng",
  "Manage and clean standard desktop apps & Windows Store apps":
    "Quản lý và dọn dẹp ứng dụng desktop và ứng dụng Windows Store",
  "Reload List": "Làm mới danh sách",
  "Export": "Xuất",
  "Export to CSV": "Xuất CSV",
  "Export to JSON": "Xuất JSON",
  "Export to HTML": "Xuất HTML",

  // Filters & sorting
  "Search apps by name or publisher...": "Tìm ứng dụng theo tên hoặc nhà phát hành...",
  "Filter by source": "Lọc theo nguồn",
  "All sources ({count})": "Tất cả nguồn ({count})",
  "Desktop ({count})": "Máy tính ({count})",
  "Store ({count})": "Store ({count})",
  "CLI packages ({count})": "Gói CLI ({count})",
  "Toolchains ({count})": "Bộ công cụ ({count})",
  "Portable ({count})": "Portable ({count})",
  "Dev only": "Chỉ ứng dụng dev",
  "Filter by developer category": "Lọc theo danh mục nhà phát triển",
  "All categories": "Tất cả danh mục",
  "Sort applications": "Sắp xếp ứng dụng",
  "Sort: Name": "Sắp xếp: Tên",
  "Sort: Size": "Sắp xếp: Dung lượng",
  "Sort: Source": "Sắp xếp: Nguồn",

  // Application table
  "No applications matched your search criteria.":
    "Không có ứng dụng nào khớp với tiêu chí tìm kiếm.",
  "Select all applications": "Chọn tất cả ứng dụng",
  "App Name": "Tên ứng dụng",
  "Publisher": "Nhà phát hành",
  "Version": "Phiên bản",
  "Size": "Dung lượng",
  "Source": "Nguồn",
  "Actions": "Thao tác",
  "Select application {name}": "Chọn ứng dụng {name}",
  "Select for bulk uninstall": "Chọn để gỡ cài đặt hàng loạt",
  "CLI/toolchain entries are uninstalled individually":
    "Mục CLI/bộ công cụ được gỡ cài đặt riêng lẻ",
  "Verified Publisher Certificate": "Chứng chỉ nhà phát hành đã xác minh",
  "ACTIVE": "ĐANG DÙNG",
  "UPDATE": "CẬP NHẬT",
  "Unknown": "Không rõ",
  "Verified": "Đã xác minh",
  "Unverified": "Chưa xác minh",

  // Row actions
  "Uninstall this global package and review owned leftovers":
    "Gỡ cài đặt gói toàn cục này và xem lại phần còn sót",
  "Uninstall package": "Gỡ cài đặt gói",
  "Uninstall this toolchain version": "Gỡ cài đặt phiên bản bộ công cụ này",
  "Uninstall": "Gỡ cài đặt",
  "Remove this unregistered portable application folder":
    "Xoá thư mục ứng dụng portable chưa đăng ký này",
  "Remove folder": "Xoá thư mục",
  "Run the registered Modify command": "Chạy lệnh Modify đã đăng ký",
  "Modify": "Sửa đổi",
  "Uninstall and Deep Clean Remnants": "Gỡ cài đặt và dọn sâu phần còn sót",
  "Remove only the orphaned uninstall registry entry":
    "Chỉ xoá mục đăng ký gỡ cài đặt bị mồ côi",
  "Remove Entry": "Xoá mục",

  // Preflight modal
  "Confirm uninstall": "Xác nhận gỡ cài đặt",
  "Review exactly what PurgeKit will run before anything is removed.":
    "Xem lại chính xác những gì PurgeKit sẽ chạy trước khi xoá bất cứ thứ gì.",
  "Close uninstall confirmation": "Đóng hộp thoại xác nhận gỡ cài đặt",
  "Estimated size": "Dung lượng ước tính",
  "Managed by": "Được quản lý bởi",
  "Registry uninstall entry": "Mục đăng ký gỡ cài đặt",
  "Install location": "Vị trí cài đặt",
  "Not registered": "Chưa đăng ký",
  "Execution method": "Cách thức thực thi",
  "Run {manager} package uninstall, then review package-owned leftovers.":
    "Chạy gỡ cài đặt gói {manager}, sau đó xem lại phần còn sót thuộc gói.",
  "Run {manager} toolchain uninstall for {version}.":
    "Chạy gỡ cài đặt bộ công cụ {manager} cho {version}.",
  "Run Remove-AppxPackage for the current user, then review leftovers.":
    "Chạy Remove-AppxPackage cho người dùng hiện tại, sau đó xem lại phần còn sót.",
  "Move the portable application folder to quarantine. No registry uninstaller is available.":
    "Chuyển thư mục ứng dụng portable vào cách ly. Không có bộ gỡ cài đặt qua registry.",
  "Run the registered uninstaller:": "Chạy bộ gỡ cài đặt đã đăng ký:",
  "no uninstall string": "không có lệnh gỡ cài đặt",
  "Administrator rights are required for this operation.":
    "Thao tác này yêu cầu quyền quản trị viên.",
  "WARNING: this toolchain is currently ACTIVE. Removing it can break existing projects.":
    "CẢNH BÁO: bộ công cụ này hiện đang hoạt động. Xoá nó có thể làm hỏng các dự án hiện có.",
  "Safety settings": "Cài đặt an toàn",
  "Reading settings…": "Đang đọc cài đặt…",
  "Backup/quarantine before delete:": "Sao lưu/cách ly trước khi xoá:",
  "On": "Bật",
  "Off": "Tắt",
  "Create restore point:": "Tạo điểm khôi phục:",
  "Backup is off. Removed files may not be restorable.":
    "Sao lưu đang tắt. Các tệp đã xoá có thể không khôi phục được.",
  "Could not read settings; backend defaults apply.":
    "Không đọc được cài đặt; dùng giá trị mặc định của backend.",
  "Force removal fallback": "Phương án xoá cưỡng bức",
  "Bypasses the official uninstaller and removes the install folder plus the uninstall registry entry. Use only when the normal uninstaller is missing or broken.":
    "Bỏ qua bộ gỡ cài đặt chính thức, xoá thư mục cài đặt và mục đăng ký gỡ cài đặt. Chỉ dùng khi bộ gỡ cài đặt thông thường bị thiếu hoặc hỏng.",
  "Force removing…": "Đang xoá cưỡng bức…",
  "Force Remove": "Xoá cưỡng bức",
  "Cancel": "Huỷ",
  "Continue": "Tiếp tục",

  // Remnants modal
  "Deep Clean remnants for:": "Dọn sâu phần còn sót cho:",
  "Review files, directories, and registry keys to be permanently purged.":
    "Xem lại các tệp, thư mục và khóa registry sẽ bị xoá vĩnh viễn.",
  "Close remnants dialog": "Đóng hộp thoại phần còn sót",
  "Official Uninstaller Running": "Bộ gỡ cài đặt chính thức đang chạy",
  "Scanning Leftover Traces": "Đang quét dấu vết còn sót",
  "Purge Completed!": "Đã dọn xong!",
  "Successfully removed": "Đã xoá thành công",
  "remnants.": "phần còn sót.",
  "Failed to remove {count} items (locked or permission issue).":
    "Không xoá được {count} mục (bị khoá hoặc thiếu quyền).",
  "No Remnants Detected!": "Không phát hiện phần còn sót!",
  "This application was successfully uninstalled without leaving residual traces.":
    "Ứng dụng này đã được gỡ cài đặt thành công mà không để lại dấu vết.",
  "Select All": "Chọn tất cả",
  "Deselect All": "Bỏ chọn tất cả",
  "{selected} of {total} items selected": "Đã chọn {selected}/{total} mục",
  "All": "Tất cả",
  "None": "Bỏ chọn",
  "Total selected:": "Tổng đã chọn:",
  "Purging remnants...": "Đang dọn phần còn sót...",
  "Purge Selected Remnants": "Dọn các phần còn sót đã chọn",
  "Folders": "Thư mục",
  "Files": "Tệp",
  "Registry keys": "Khóa registry",
  "Registry values": "Giá trị registry",
  "Scheduled tasks": "Tác vụ theo lịch",
  "Task references this app or its install path":
    "Tác vụ tham chiếu ứng dụng này hoặc đường dẫn cài đặt của nó",
  "Registered uninstall entry": "Mục gỡ cài đặt đã đăng ký",
  "Name/path-matched registry trace": "Dấu vết registry khớp tên/đường dẫn",
  "Strong install-location match": "Khớp chặt với vị trí cài đặt",
  "Name-matched leftover in a scanned location":
    "Phần còn sót khớp tên trong vị trí đã quét",

  // Bulk action bar
  "{count} apps selected": "Đã chọn {count} ứng dụng",
  "Ready for batch silent uninstallation": "Sẵn sàng gỡ cài đặt im lặng hàng loạt",
  "{count} hidden ignored": "{count} mục ẩn bị bỏ qua",
  "Auto-Purge Leftovers": "Tự động dọn phần còn sót",
  "Delete remnants silently": "Xoá phần còn sót không hỏi lại",
  "Force Fallback": "Phương án cưỡng bức",
  "If uninstaller fails": "Nếu bộ gỡ cài đặt thất bại",
  "Bulk Silent Uninstall": "Gỡ cài đặt im lặng hàng loạt",

  // Bulk uninstall modal
  "Bulk Silent Uninstallation": "Gỡ cài đặt im lặng hàng loạt",
  "Processing": "Đang xử lý",
  "Review remaining leftovers from batch uninstallation":
    "Xem lại phần còn sót từ lần gỡ cài đặt hàng loạt",
  "Batch operations completed": "Đã hoàn tất thao tác hàng loạt",
  "Close bulk uninstallation dialog": "Đóng hộp thoại gỡ cài đặt hàng loạt",
  "Uninstall Log": "Nhật ký gỡ cài đặt",
  "Remaining Leftovers Detected": "Phát hiện phần còn sót",
  "The uninstaller completed, but some confidence-level remnants couldn't be auto-purged (or auto-purge was disabled). Review and select below to clean them permanently.":
    "Bộ gỡ cài đặt đã chạy xong, nhưng một số phần còn sót chưa được tự động dọn (hoặc tính năng tự động dọn đang tắt). Hãy xem và chọn bên dưới để xoá vĩnh viễn.",
  "Bulk Uninstall Failed": "Gỡ cài đặt hàng loạt thất bại",
  "The operation could not be completed.": "Không thể hoàn tất thao tác.",
  "Some applications may have been processed before the error. The list was refreshed.":
    "Một số ứng dụng có thể đã được xử lý trước khi xảy ra lỗi. Danh sách đã được làm mới.",
  "All Applications Processed!": "Đã xử lý tất cả ứng dụng!",
  "Bulk uninstallation and leftovers cleaning completed successfully.":
    "Đã gỡ cài đặt hàng loạt và dọn phần còn sót thành công.",
  "Skip Cleaning": "Bỏ qua dọn dẹp",
  "Purging leftovers...": "Đang dọn phần còn sót...",
  "Clean Leftover Traces": "Dọn dấu vết còn sót",
  "Done": "Xong",

  // Status messages, toasts & confirmations
  "Scanning applications, Store packages & developer tools...":
    "Đang quét ứng dụng, gói Store và công cụ lập trình...",
  "Error scanning application list!": "Lỗi khi quét danh sách ứng dụng!",
  "Executing the official uninstaller. Please follow the uninstallation GUI prompts on your screen...":
    "Đang chạy bộ gỡ cài đặt chính thức. Vui lòng làm theo hướng dẫn trên màn hình...",
  "The official uninstaller returned an error or was closed.":
    "Bộ gỡ cài đặt chính thức trả về lỗi hoặc đã bị đóng.",
  "Scanning system directories and registry paths for leftover traces...":
    "Đang quét các thư mục hệ thống và đường dẫn registry để tìm dấu vết còn sót...",
  "Error scanning remnant files on the system!":
    "Lỗi khi quét tệp còn sót trên hệ thống!",
  "Uninstall global {manager} package \"{name}\"?":
    "Gỡ cài đặt gói {manager} toàn cục \"{name}\"?",
  "Uninstalling {name} via {manager}...": "Đang gỡ cài đặt {name} qua {manager}...",
  "Scanning package-owned paths for {name}...":
    "Đang quét các đường dẫn thuộc gói cho {name}...",
  "Package uninstall failed: {error}": "Gỡ cài đặt gói thất bại: {error}",
  "WARNING: {version} is the ACTIVE {manager} toolchain. Uninstalling it can break existing projects.":
    "CẢNH BÁO: {version} là bộ công cụ {manager} đang hoạt động. Gỡ cài đặt nó có thể làm hỏng các dự án hiện có.",
  "Uninstall {manager} toolchain \"{version}\"?{warning}":
    "Gỡ cài đặt bộ công cụ {manager} \"{version}\"?{warning}",
  "Uninstalled {manager} {version}.": "Đã gỡ cài đặt {manager} {version}.",
  "Toolchain uninstall failed: {error}": "Gỡ cài đặt bộ công cụ thất bại: {error}",
  "FORCE REMOVE \"{name}\"?\n\nThis bypasses the official uninstaller and removes:\n- install folder: {path}\n- uninstall registry entry: {registry}\n\nFiles are quarantined if backup is enabled; registry is exported first.":
    "XOÁ CƯỠNG BỨC \"{name}\"?\n\nThao tác này bỏ qua bộ gỡ cài đặt chính thức và xoá:\n- thư mục cài đặt: {path}\n- mục đăng ký gỡ cài đặt: {registry}\n\nTệp sẽ được cách ly nếu bật sao lưu; registry được xuất trước.",
  "(none)": "(không có)",
  "(not applicable)": "(không áp dụng)",
  "Force remove finished with {count} failure(s): {error}":
    "Xoá cưỡng bức xong với {count} lỗi: {error}",
  "see logs": "xem nhật ký",
  "Force removed {name}.": "Đã xoá cưỡng bức {name}.",
  "Force remove failed: {error}": "Xoá cưỡng bức thất bại: {error}",
  "Run the registered Modify command for \"{name}\"?\n\n{path}":
    "Chạy lệnh Modify đã đăng ký cho \"{name}\"?\n\n{path}",
  "Modify command finished for {name}.": "Đã chạy xong lệnh Modify cho {name}.",
  "Modify command failed: {error}": "Lệnh Modify thất bại: {error}",
  "Remove the uninstall entry for \"{name}\"?\n\nRegistry key: {path}\n\nThis only removes the registry record. Files are NOT deleted.":
    "Xoá mục gỡ cài đặt cho \"{name}\"?\n\nKhóa registry: {path}\n\nThao tác này chỉ xoá bản ghi registry. Các tệp KHÔNG bị xoá.",
  "Removed the uninstall entry for {name}.":
    "Đã xoá mục gỡ cài đặt cho {name}.",
  "Failed to remove entry: {error}": "Xoá mục thất bại: {error}",
  "Please select at least one remnant item to clean.":
    "Vui lòng chọn ít nhất một mục còn sót để dọn.",
  "Purge completed! Successfully removed {count} remnant items.":
    "Đã dọn xong! Đã xoá thành công {count} mục còn sót.",
  "Error purging remnants: {error}": "Lỗi khi dọn phần còn sót: {error}",
  "No application data to export!": "Không có dữ liệu ứng dụng để xuất!",

  // Exported HTML report
  "PurgeKit application report": "Báo cáo ứng dụng PurgeKit",
  "{count} applications · generated {date}": "{count} ứng dụng · tạo lúc {date}",
  "Name": "Tên",
  "Dev / Categories": "Dev / Danh mục",
  "Signature": "Chữ ký",
  "Location": "Vị trí",

  // Bulk uninstall toasts
  "Successfully exported report to {format}!":
    "Đã xuất báo cáo sang {format} thành công!",
  "Initializing bulk silent uninstallation for {count} application(s)...":
    "Đang khởi tạo gỡ cài đặt im lặng hàng loạt cho {count} ứng dụng...",
  "Bulk uninstallation and auto-purge completed! No remaining leftovers found.":
    "Đã gỡ cài đặt hàng loạt và tự động dọn xong! Không còn phần còn sót.",
  "Bulk uninstallation error: {error}": "Lỗi gỡ cài đặt hàng loạt: {error}",
  "No leftovers selected to clean. Closing.":
    "Chưa chọn phần còn sót nào để dọn. Đang đóng.",
  "Purge completed! Removed {count} leftover items.":
    "Đã dọn xong! Đã xoá {count} mục còn sót.",
  "Error purging leftovers: {error}": "Lỗi khi dọn phần còn sót: {error}",
  "Restart PurgeKit as Administrator? The current window will close.":
    "Khởi động lại PurgeKit bằng quyền Quản trị? Cửa sổ hiện tại sẽ đóng.",
  "Could not restart as Administrator: {error}":
    "Không thể khởi động lại bằng quyền Quản trị: {error}",
  "Some selected applications require Administrator privileges. Restart PurgeKit as Administrator and try again.":
    "Một số ứng dụng đã chọn cần quyền Quản trị. Hãy khởi động lại PurgeKit bằng quyền Quản trị rồi thử lại.",
  "Administrator rights are required. Restart PurgeKit as Administrator to continue.":
    "Cần quyền Quản trị. Hãy khởi động lại PurgeKit bằng quyền Quản trị để tiếp tục.",
  "Restart as Administrator": "Khởi động lại bằng quyền Quản trị",
  "Uninstaller did not complete": "Trình gỡ cài đặt chưa hoàn tất",
  "No leftovers were detected, but the uninstaller reported an error. Verify the app is gone before retrying.":
    "Không phát hiện phần còn sót, nhưng trình gỡ cài đặt báo lỗi. Hãy kiểm tra ứng dụng đã biến mất trước khi thử lại.",
  "The uninstaller did not complete. Leftovers are shown for review but are not pre-selected; only purge items you are sure about.":
    "Trình gỡ cài đặt chưa hoàn tất. Các phần còn sót được hiển thị để xem nhưng không được chọn sẵn; chỉ dọn những mục bạn chắc chắn.",
  "Some leftovers were not auto-purged, or an uninstaller did not complete. Review each item below before cleaning it permanently.":
    "Một số phần còn sót chưa được tự động dọn, hoặc trình gỡ cài đặt chưa hoàn tất. Hãy xem từng mục bên dưới trước khi xoá vĩnh viễn.",
};

export default apps;
