//! 多显示器 × 多用户的 QoS 状态机。
//!
//! 上游 `VideoQOS` 的 `fps` / `ratio` 是**全局标量**，多显示器同时采集时
//! 两块屏共用同一组值：一块屏拥塞会把另一块也拖慢，一个人要「最佳画质」
//! 会让所有人的码率都涨上去。本项目每块屏有独立的采集器与编码器，
//! QoS 必须与之一一对应。
//!
//! 本测试锁定三条聚合规则：
//!
//! | 量 | 规则 | 理由 |
//! |---|---|---|
//! | `ratio` | 同一屏取**最大** | 有人要「最佳」就不该被别人拉低 |
//! | `target_fps` | 同一屏取**最小** | 编码器只有一个，要满足最慢的控制端 |
//! | `fps` | 同一屏取**最小** | 任一链路拥塞就该降，否则那块屏持续积压 |
//!
//! 运行：cargo test --release --test qos_multidisplay -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_qos.rs"]
mod sos_qos;

use sos_qos::{SosQos, BR_BALANCED, BR_BEST, BR_LOW, DEFAULT_FPS, MIN_FPS, QUEUE_BAD_MS};

const DP0: &str = r"\\.\DISPLAY1";
const DP1: &str = r"\\.\DISPLAY2";

/// 制造一次拥塞样本（连续两次才降帧）
fn congest(q: &mut SosQos, id: i32) {
    q.on_test_delay(id, 5, QUEUE_BAD_MS + 20);
    q.on_test_delay(id, 5, QUEUE_BAD_MS + 20);
}

/// **核心**：一块屏拥塞不得影响另一块
#[test]
fn one_congested_display_does_not_slow_the_other() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP1);

    // 两条链路都健康
    assert_eq!(q.fps(DP0), DEFAULT_FPS);
    assert_eq!(q.fps(DP1), DEFAULT_FPS);

    // 只有看 DP0 的连接拥塞
    congest(&mut q, 1);

    let fps0 = q.fps(DP0);
    let fps1 = q.fps(DP1);
    println!("DP0 fps={}  DP1 fps={}", fps0, fps1);
    assert!(fps0 < DEFAULT_FPS, "拥塞的屏必须降帧，实际 {}", fps0);
    assert_eq!(fps1, DEFAULT_FPS, "健康的屏不能受影响");
    // 帧间隔也要跟着分叉
    assert!(q.spf(DP0) > q.spf(DP1));
}

/// **多用户看同一块屏**：取最小 fps，任一链路拥塞就整体降
#[test]
fn same_display_viewers_take_the_worst_fps() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP0);

    assert_eq!(q.fps(DP0), DEFAULT_FPS);
    congest(&mut q, 2);

    let fps = q.fps(DP0);
    println!("两个控制端看同一屏，其中之一拥塞 → fps={}", fps);
    assert!(fps < DEFAULT_FPS, "任一看者拥塞，该屏就该降帧");

    // 拥塞者断开后，聚合结果应自然回到健康值
    q.on_connection_close(2);
    let fps_after = q.fps(DP0);
    println!("拥塞者断开后 fps={}", fps_after);
    assert_eq!(
        fps_after, DEFAULT_FPS,
        "最慢的连接离开后必须立刻恢复（min 聚合的自然结果）"
    );
}

/// **画质取最大**：有人要「最佳」，就不该被别人的「低画质」拉低
///
/// 注意「未表态」的连接按默认档（Balanced）参与聚合：一个从没设置过画质的
/// 控制端期望看到默认画质，所以它会把别人的 Low 抬回 Balanced。
/// 这不是缺陷，而是「不能说降就降」的保守语义。
#[test]
fn same_display_takes_best_quality() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP0);

    assert!((q.ratio(DP0) - BR_BALANCED).abs() < 1e-6, "默认是平衡画质");

    // 只有一个人要 Low：另一人未表态（=Balanced），所以结果仍是 Balanced
    q.set_image_quality(1, 2); // Low
    assert!(
        (q.ratio(DP0) - BR_BALANCED).abs() < 1e-6,
        "未表态的连接按默认档参与聚合"
    );

    // 两个人都要 Low → Low
    q.set_image_quality(2, 2);
    assert!((q.ratio(DP0) - BR_LOW).abs() < 1e-6);

    // 其中一人改成 Best → 取 Best
    q.set_image_quality(2, 4);
    let r = q.ratio(DP0);
    println!("Low + Best → ratio={}", r);
    assert!((r - BR_BEST).abs() < 1e-6, "应取两者中更高的画质");

    // 高位者断开，退回剩下的 Low
    q.on_connection_close(2);
    assert!((q.ratio(DP0) - BR_LOW).abs() < 1e-6);
}

/// **画质按屏独立**：改 DP0 的画质不影响 DP1
#[test]
fn quality_is_per_display() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP1);
    q.set_image_quality(1, 4); // DP0 → Best
    assert!((q.ratio(DP0) - BR_BEST).abs() < 1e-6);
    assert!(
        (q.ratio(DP1) - BR_BALANCED).abs() < 1e-6,
        "DP1 应保持默认画质"
    );
    println!("DP0={} DP1={}", q.ratio(DP0), q.ratio(DP1));
}

/// **帧率上限取最小**：一个人要求 10fps，则该屏最多 10fps
#[test]
fn custom_fps_cap_takes_the_lowest() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP0);

    q.set_custom_fps(2, 10);
    assert_eq!(q.target_fps(DP0), 10);
    assert_eq!(q.fps(DP0), 10, "当前帧率不得超过该上限");

    // 越界值被忽略
    q.set_custom_fps(1, 0);
    q.set_custom_fps(1, 999);
    assert_eq!(q.target_fps(DP0), 10, "非法上限不应生效");

    // 放宽后恢复
    q.on_connection_close(2);
    assert_eq!(q.target_fps(DP0), DEFAULT_FPS);
}

/// **显示器切换**：连接从 DP0 移到 DP1，两边的聚合都要跟着变
#[test]
fn switching_display_moves_aggregation() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP1);
    q.set_image_quality(1, 4); // DP0 = Best
    q.set_custom_fps(1, 12);

    assert!(q.set_connection_display(1, DP1), "应报告显示器确实变了");
    assert!(!q.set_connection_display(1, DP1), "重复切换应返回 false");

    println!(
        "切换后 DP0 ratio={} fps={} | DP1 ratio={} fps={}",
        q.ratio(DP0),
        q.fps(DP0),
        q.ratio(DP1),
        q.fps(DP1)
    );
    // DP0 已无人观看 → 回落到默认值
    assert!((q.ratio(DP0) - BR_BALANCED).abs() < 1e-6);
    assert_eq!(q.fps(DP0), DEFAULT_FPS);
    // DP1 现在有「Best + 12fps 上限」的看者
    assert!((q.ratio(DP1) - BR_BEST).abs() < 1e-6);
    assert_eq!(q.target_fps(DP1), 12);
}

/// **帧率自身**：按比例平滑升降，既不一跳到底也不停滞
#[test]
fn fps_moves_gradually_in_both_directions() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);

    println!("{:>3} {:>6}", "样本", "fps");
    println!("{:>3} {:>6}", 0, q.fps(DP0));

    // 连续拥塞：每 2 个样本降一档
    let mut trace = vec![q.fps(DP0)];
    for i in 1..=10 {
        q.on_test_delay(1, 5, QUEUE_BAD_MS + 50);
        if i % 2 == 0 {
            trace.push(q.fps(DP0));
        }
    }
    println!("拥塞序列: {:?}", trace);
    assert_eq!(*trace.last().unwrap(), MIN_FPS, "持续拥塞应到达下限");
    // 不能一次就到底
    assert!(
        trace[0] > trace[1] && trace[1] < DEFAULT_FPS,
        "应是逐步下降而非一步到底：{:?}",
        trace
    );

    // 连续良好：逐档回升
    let mut up = vec![q.fps(DP0)];
    for _ in 0..10 {
        q.on_test_delay(1, 5, 0);
        up.push(q.fps(DP0));
    }
    println!("恢复序列: {:?}", up);
    assert_eq!(*up.last().unwrap(), DEFAULT_FPS, "应恢复到上限");
    assert!(up.len() > 2 && up[1] > up[0], "应逐档回升");
}

/// **高 RTT 但无排队不降帧**：卫星 / 跨洋链路的瓶颈是传播时延，降帧无益
#[test]
fn high_rtt_without_queue_keeps_fps() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    for _ in 0..6 {
        q.on_test_delay(1, 600, 0); // RTT 600ms，但排队 0
    }
    println!("rtt=600 queue=0 → fps={}", q.fps(DP0));
    assert_eq!(q.fps(DP0), DEFAULT_FPS, "无排队说明带宽够，不该降帧");

    // 反过来：RTT 很低但排队严重 → 必须降
    for _ in 0..4 {
        q.on_test_delay(1, 5, QUEUE_BAD_MS + 100);
    }
    println!("rtt=5 queue={} → fps={}", QUEUE_BAD_MS + 100, q.fps(DP0));
    assert!(q.fps(DP0) < DEFAULT_FPS, "低 RTT 高排队是 Wi-Fi 典型特征");
}

/// **码率登记按屏独立**
#[test]
fn bitrate_is_stored_per_display() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP1);
    q.store_bitrate(DP0, 2326);
    q.store_bitrate(DP1, 1163);
    assert_eq!(q.bitrate_kbps(DP0), 2326);
    assert_eq!(q.bitrate_kbps(DP1), 1163);
    // 未知屏返回 0，而不是别的屏的值
    assert_eq!(q.bitrate_kbps(r"\\.\DISPLAY9"), 0);
    println!("DP0={} DP1={}", q.bitrate_kbps(DP0), q.bitrate_kbps(DP1));
}

/// **快照一致性**：一次取走的各字段必须来自同一时刻
#[test]
fn display_snapshot_is_consistent() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP0);
    q.store_bitrate(DP0, 3000);
    q.set_image_quality(2, 4);
    q.on_test_delay(1, 30, 0);

    let s = q.display_snapshot(DP0);
    println!(
        "viewers={} fps={}/{} ratio={} bitrate={} rtt={:?} queue={:?}",
        s.viewers, s.fps, s.target_fps, s.ratio, s.bitrate_kbps, s.worst_rtt_ms, s.worst_queue_ms
    );
    assert_eq!(s.viewers, 2);
    assert_eq!(s.bitrate_kbps, 3000);
    assert!((s.ratio - BR_BEST).abs() < 1e-6);
    assert_eq!(s.worst_rtt_ms, Some(30));
    assert_eq!(s.worst_queue_ms, Some(0));

    // 无人观看的屏返回默认值而不是 panic
    let empty = q.display_snapshot(r"\\.\DISPLAY9");
    assert_eq!(empty.viewers, 0);
    assert_eq!(empty.fps, DEFAULT_FPS);
}

/// **未知连接**：不要 panic，也不要污染别人的状态
#[test]
fn unknown_connection_is_ignored() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.set_image_quality(1, 4);
    let before = q.ratio(DP0);

    q.set_image_quality(99, 2);
    q.set_custom_fps(99, 10);
    q.on_test_delay(99, 5, QUEUE_BAD_MS + 50);
    q.on_connection_close(99);
    assert!(!q.set_connection_display(99, DP1));

    assert!((q.ratio(DP0) - before).abs() < 1e-6, "未知连接不得改画质");
    assert_eq!(q.target_fps(DP0), DEFAULT_FPS, "未知连接不得改帧率上限");
    assert_eq!(q.fps(DP0), DEFAULT_FPS, "未知连接不得改自适应帧率");
    println!("未知连接的所有调用均被安全忽略");
}

/// **三屏三用户**：各自独立演进
#[test]
fn three_displays_three_viewers() {
    const DP2: &str = r"\\.\DISPLAY3";
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.on_connection_open(2, DP1);
    q.on_connection_open(3, DP2);

    q.set_image_quality(1, 2); // DP0 Low
    q.set_image_quality(2, 3); // DP1 Balanced
    q.set_image_quality(3, 4); // DP2 Best
    congest(&mut q, 2); // 只有 DP1 拥塞

    println!(
        "DP0 ratio={:.2} fps={} | DP1 ratio={:.2} fps={} | DP2 ratio={:.2} fps={}",
        q.ratio(DP0),
        q.fps(DP0),
        q.ratio(DP1),
        q.fps(DP1),
        q.ratio(DP2),
        q.fps(DP2)
    );
    assert!((q.ratio(DP0) - BR_LOW).abs() < 1e-6);
    assert!((q.ratio(DP1) - BR_BALANCED).abs() < 1e-6);
    assert!((q.ratio(DP2) - BR_BEST).abs() < 1e-6);
    assert_eq!(q.fps(DP0), DEFAULT_FPS);
    assert!(q.fps(DP1) < DEFAULT_FPS, "DP1 拥塞");
    assert_eq!(q.fps(DP2), DEFAULT_FPS);
}


// ── 画质选项是否真的生效 ──
//
// 生产问题四：控制端调「画质」完全没反应，码率始终停在 7500kbps。
// 根因不在 QoS，而在 `sos_connection` 里有一层「VP9 就强制抬到 Best」的钳制：
// 默认链路是 VP9 ⇒ Low / Balanced / 自定义值全被改写成 Best。
//
// 这里锁定**协议值 → 画质比**的映射本身（那层钳制已删除，映射必须与上游一致）。

#[test]
fn image_quality_maps_to_distinct_ratios() {
    use sos_qos::ratio_of_image_quality;

    // 三档必须给出三个**不同**的值。若相同，控制端切档就观察不到任何变化。
    let low = ratio_of_image_quality(2).expect("Low 必须解析出画质比");
    let balanced = ratio_of_image_quality(3).expect("Balanced 必须解析出画质比");
    let best = ratio_of_image_quality(4).expect("Best 必须解析出画质比");
    assert!(low < balanced && balanced < best, "三档必须严格递增");
    assert!((low - BR_LOW).abs() < 1e-6);
    assert!((balanced - BR_BALANCED).abs() < 1e-6);
    assert!((best - BR_BEST).abs() < 1e-6);

    // NotSet = 不改变当前值
    assert_eq!(ratio_of_image_quality(0), None, "NotSet 不能覆写当前画质");

    // 自定义值：上游客户端发的是 `quality << 8`（quality 为百分比），
    // 例如 80 -> 20480 -> 1.6 倍
    let custom = ratio_of_image_quality(80 << 8).expect("自定义画质必须解析出画质比");
    assert!((custom - 1.6).abs() < 1e-6, "80% 应解析为 1.6 倍，实际 {}", custom);

    // 负值非法
    assert_eq!(ratio_of_image_quality(-1), None);
}

/// 画质必须真的改变码率 —— 这是「调节选项有效」的可观测判据。
#[test]
fn image_quality_changes_bitrate() {
    let mut q = SosQos::new();
    q.on_connection_open(1, DP0);
    q.set_image_quality(1, 4); // Best
    let best = q.ratio(DP0);
    q.set_image_quality(1, 2); // Low
    let low = q.ratio(DP0);

    println!("Best ratio={:.2}  Low ratio={:.2}", best, low);
    assert!(low < best, "切到 Low 之后画质比必须下降");
    assert!((best - BR_BEST).abs() < 1e-6);
    assert!((low - BR_LOW).abs() < 1e-6);
}
