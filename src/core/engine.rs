//! Stockfish engine integration via UCI protocol
//!
//! This module handles communication with the Stockfish engine embedded in the binary.
//! The engine is extracted to a temp directory at startup and communicates via stdin/stdout.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use vampirc_uci::{UciInfoAttribute, UciMessage};

use crate::core::config::EngineSettings;
use crate::metadata;
use crate::storage::write_atomic;

/// Embedded Stockfish binary (included at compile time)
const STOCKFISH_BINARY: &[u8] = include_bytes!("../../assets/stockfish");

/// A single analysis line with its evaluation
#[derive(Debug, Clone, Default)]
pub struct AnalysisLine {
    /// Line number (1 = best, 2 = second best, etc.)
    pub multipv: u32,
    /// Centipawn score (positive = white advantage)
    pub score_cp: Option<i32>,
    /// Mate in N moves (positive = white mates, negative = black mates)
    pub mate_in: Option<i32>,
    /// Principal variation moves
    pub pv: Vec<String>,
    /// Search depth for this line
    pub depth: u32,
}

/// Complete engine analysis result
#[derive(Debug, Clone, Default)]
pub struct EngineAnalysis {
    /// All analysis lines (sorted by multipv)
    pub lines: Vec<AnalysisLine>,
    /// Best move determined by engine (available after "bestmove" response)
    pub best_move: Option<String>,
    /// Current overall depth
    pub depth: u32,
}

impl EngineAnalysis {
    /// Get the best (first) line
    pub fn best_line(&self) -> Option<&AnalysisLine> {
        self.lines.first()
    }
}

/// Engine state and communication
pub struct Engine {
    _process: Child,
    stdin: ChildStdin,
    analysis_receiver: Receiver<EngineAnalysis>,
    current_analysis: EngineAnalysis,
    /// Track whose turn it is for score perspective
    white_to_move: bool,
    /// Max depth setting (None = infinite)
    max_depth: Option<u32>,
}

impl Engine {
    /// Start the engine with settings
    pub fn new_with_settings(settings: &EngineSettings) -> Result<Self, String> {
        let stockfish_path = extract_stockfish()?;

        let mut process = Command::new(&stockfish_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start Stockfish: {e}"))?;

        let stdin = process.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = process.stdout.take().ok_or("Failed to get stdout")?;

        // Channel for receiving analysis from reader thread
        let (tx, rx) = mpsc::channel();

        // Spawn reader thread
        spawn_reader_thread(stdout, tx, settings.multi_pv);

        let mut engine = Self {
            _process: process,
            stdin,
            analysis_receiver: rx,
            current_analysis: EngineAnalysis::default(),
            white_to_move: true,
            max_depth: settings.max_depth,
        };

        // Initialize UCI
        engine.send_command("uci")?;
        engine.send_command(&format!(
            "setoption name MultiPV value {}",
            settings.multi_pv
        ))?;
        engine.send_command(&format!(
            "setoption name Threads value {}",
            settings.threads
        ))?;
        engine.send_command(&format!("setoption name Hash value {}", settings.hash_mb))?;
        engine.send_command("isready")?;

        Ok(engine)
    }

    /// Send a command to the engine
    pub fn send_command(&mut self, cmd: &str) -> Result<(), String> {
        writeln!(self.stdin, "{}", cmd).map_err(|e| format!("Failed to send command: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))?;
        Ok(())
    }

    /// Set the position using FEN and track whose turn it is
    pub fn set_position(&mut self, fen: &str, is_white_turn: bool) -> Result<(), String> {
        self.white_to_move = is_white_turn;
        self.clear_analysis();
        self.send_command(&format!("position fen {}", fen))
    }

    /// Clear current analysis
    pub fn clear_analysis(&mut self) {
        self.current_analysis = EngineAnalysis::default();
        while self.analysis_receiver.try_recv().is_ok() {}
    }

    /// Start infinite analysis
    pub fn go_infinite(&mut self) -> Result<(), String> {
        self.send_command("go infinite")
    }

    /// Start analysis with configured depth limit (or infinite if None)
    pub fn go(&mut self) -> Result<(), String> {
        match self.max_depth {
            Some(depth) => self.send_command(&format!("go depth {}", depth)),
            None => self.go_infinite(),
        }
    }

    /// Start analysis to a specific depth
    pub fn go_depth(&mut self, depth: u32) -> Result<(), String> {
        self.send_command(&format!("go depth {}", depth))
    }

    /// Stop current analysis
    pub fn stop(&mut self) -> Result<(), String> {
        self.send_command("stop")
    }

    /// Analyze a single position to a fixed depth.
    pub fn analyze_position_depth(
        &mut self,
        fen: &str,
        is_white_turn: bool,
        depth: u32,
        timeout_ms: u64,
    ) -> Result<EngineAnalysis, String> {
        let _ = self.stop();
        self.set_position(fen, is_white_turn)?;
        self.go_depth(depth)?;

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut latest: Option<EngineAnalysis> = None;

        while Instant::now() < deadline {
            if let Some(a) = self.poll_analysis() {
                let snapshot = a.clone();
                latest = Some(snapshot.clone());
                if snapshot.depth >= depth {
                    let _ = self.stop();
                    return Ok(snapshot);
                }
            }
            thread::sleep(Duration::from_millis(12));
        }

        let _ = self.stop();
        latest.ok_or_else(|| "Engine review timed out".to_string())
    }

    /// Poll for new analysis (non-blocking)
    /// Returns analysis from WHITE's perspective (positive = white is better)
    pub fn poll_analysis(&mut self) -> Option<&EngineAnalysis> {
        while let Ok(mut analysis) = self.analysis_receiver.try_recv() {
            // Convert scores to white's perspective
            if !self.white_to_move {
                for line in &mut analysis.lines {
                    line.score_cp = line.score_cp.map(|cp| -cp);
                    line.mate_in = line.mate_in.map(|m| -m);
                }
            }
            self.current_analysis = analysis;
        }

        if self.current_analysis.depth > 0 {
            Some(&self.current_analysis)
        } else {
            None
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.send_command("quit");
    }
}

/// Extract the embedded Stockfish binary to a temp file
fn extract_stockfish() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let stockfish_path = temp_dir.join(format!(
        "{}-stockfish-{:016x}",
        metadata::APP_ID,
        stockfish_content_hash()
    ));

    let should_extract = match std::fs::metadata(&stockfish_path) {
        Ok(meta) => meta.len() != STOCKFISH_BINARY.len() as u64,
        Err(_) => true,
    };

    if should_extract {
        write_atomic(&stockfish_path, STOCKFISH_BINARY)
            .map_err(|e| format!("Failed to extract Stockfish: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&stockfish_path)
                .map_err(|e| format!("Failed to get metadata: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stockfish_path, perms)
                .map_err(|e| format!("Failed to set permissions: {e}"))?;
        }
    }

    Ok(stockfish_path)
}

fn stockfish_content_hash() -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in STOCKFISH_BINARY {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Spawn a thread to read engine output and collect multi-PV analysis
fn spawn_reader_thread(stdout: ChildStdout, tx: Sender<EngineAnalysis>, multipv: u32) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut current_lines: Vec<AnalysisLine> = vec![AnalysisLine::default(); multipv as usize];
        let mut current_depth = 0u32;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            // Parse using vampirc-uci
            let msg = vampirc_uci::parse_one(&line);

            match msg {
                UciMessage::Info(attrs) => {
                    let mut analysis_line = AnalysisLine::default();
                    let mut multipv_idx = 0; // Default to 0 (1st line)

                    for attr in attrs {
                        match attr {
                            UciInfoAttribute::Depth(d) => analysis_line.depth = d as u32,
                            UciInfoAttribute::MultiPv(m) => {
                                multipv_idx = (m.saturating_sub(1)) as usize;
                                analysis_line.multipv = m as u32;
                            }
                            UciInfoAttribute::Score { cp, mate, .. } => {
                                analysis_line.score_cp = cp;
                                analysis_line.mate_in = mate.map(|m| m as i32);
                            }
                            UciInfoAttribute::Pv(moves) => {
                                analysis_line.pv = moves.iter().map(|m| m.to_string()).collect();
                            }
                            _ => {}
                        }
                    }

                    // Only update if we have meaningful data (PV with score)
                    // This filters out status info lines like "currmove" that have depth but no PV/score
                    if !analysis_line.pv.is_empty()
                        && (analysis_line.score_cp.is_some() || analysis_line.mate_in.is_some())
                        && multipv_idx < current_lines.len()
                    {
                        current_depth = current_depth.max(analysis_line.depth);
                        current_lines[multipv_idx] = analysis_line;

                        // Send update
                        let analysis = EngineAnalysis {
                            lines: current_lines
                                .iter()
                                .filter(|l| l.depth > 0 || !l.pv.is_empty())
                                .cloned()
                                .collect(),
                            best_move: None,
                            depth: current_depth,
                        };
                        let _ = tx.send(analysis);
                    }
                }
                UciMessage::BestMove { best_move, .. } => {
                    let analysis = EngineAnalysis {
                        lines: current_lines
                            .iter()
                            .filter(|l| l.depth > 0 || !l.pv.is_empty())
                            .cloned()
                            .collect(),
                        best_move: Some(best_move.to_string()),
                        depth: current_depth,
                    };
                    let _ = tx.send(analysis);
                }
                _ => {}
            }
        }
    });
}
