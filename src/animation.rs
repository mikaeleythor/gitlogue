use crate::git::{CommitMetadata, DiffHunk, FileChange, LineChangeType};
use std::time::{Duration, Instant};

/// Represents the current state of the editor buffer
#[derive(Debug, Clone)]
pub struct EditorBuffer {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
}

impl EditorBuffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
        }
    }

    pub fn from_content(content: &str) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|s| s.to_string()).collect()
        };

        Self {
            lines,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
        }
    }

    pub fn insert_char(&mut self, line: usize, col: usize, ch: char) {
        if line >= self.lines.len() {
            self.lines.resize(line + 1, String::new());
        }
        self.lines[line].insert(col, ch);
    }

    pub fn delete_char(&mut self, line: usize, col: usize) {
        if line < self.lines.len() && col < self.lines[line].len() {
            self.lines[line].remove(col);
        }
    }

    pub fn insert_line(&mut self, line: usize, content: String) {
        if line > self.lines.len() {
            self.lines.resize(line, String::new());
        }
        self.lines.insert(line, content);
    }

    pub fn delete_line(&mut self, line: usize) {
        if line < self.lines.len() {
            self.lines.remove(line);
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
    }

    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }
}

/// Individual animation step
#[derive(Debug, Clone)]
pub enum AnimationStep {
    InsertChar { line: usize, col: usize, ch: char },
    DeleteChar { line: usize, col: usize },
    InsertLine { line: usize, content: String },
    DeleteLine { line: usize },
    MoveCursor { line: usize, col: usize },
    Pause { duration_ms: u64 },
    SwitchFile { file_index: usize, content: String },
}

/// Animation state machine
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationState {
    Idle,
    Playing,
    Paused,
    Finished,
}

/// Main animation engine
pub struct AnimationEngine {
    pub buffer: EditorBuffer,
    pub state: AnimationState,
    steps: Vec<AnimationStep>,
    current_step: usize,
    last_update: Instant,
    speed_ms: u64,
    pause_until: Option<Instant>,
    pub cursor_visible: bool,
    cursor_blink_timer: Instant,
    viewport_height: usize,
    pub current_file_index: usize,
}

impl AnimationEngine {
    pub fn new(speed_ms: u64) -> Self {
        Self {
            buffer: EditorBuffer::new(),
            state: AnimationState::Idle,
            steps: Vec::new(),
            current_step: 0,
            last_update: Instant::now(),
            speed_ms,
            pause_until: None,
            cursor_visible: true,
            cursor_blink_timer: Instant::now(),
            viewport_height: 20, // Default, will be updated from UI
            current_file_index: 0,
        }
    }

    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
    }

    /// Load a commit and generate animation steps
    pub fn load_commit(&mut self, metadata: &CommitMetadata) {
        self.steps.clear();
        self.current_step = 0;
        self.state = AnimationState::Playing;
        self.current_file_index = 0;

        // Process all file changes
        for (index, change) in metadata.changes.iter().enumerate() {
            // Add file switch step
            let content = change.old_content.clone().unwrap_or_default();
            self.steps.push(AnimationStep::SwitchFile {
                file_index: index,
                content: content.clone(),
            });

            // Add pause before starting file animation
            self.steps.push(AnimationStep::Pause { duration_ms: 1000 });

            // Generate animation steps for this file
            self.generate_steps_for_file(change);

            // Add pause between files
            if index < metadata.changes.len() - 1 {
                self.steps.push(AnimationStep::Pause { duration_ms: 2000 });
            }
        }

        // Final pause
        self.steps.push(AnimationStep::Pause { duration_ms: 3000 });

        // Start with the first file's content
        if let Some(change) = metadata.changes.first() {
            if let Some(old_content) = &change.old_content {
                self.buffer = EditorBuffer::from_content(old_content);
            } else {
                self.buffer = EditorBuffer::new();
            }
        }
    }

    /// Generate animation steps for a file change
    fn generate_steps_for_file(&mut self, change: &FileChange) {
        // Process each hunk
        for hunk in &change.hunks {
            // Move cursor to the start of the hunk
            self.steps.push(AnimationStep::MoveCursor {
                line: hunk.old_start,
                col: 0,
            });
            self.steps.push(AnimationStep::Pause { duration_ms: 500 });

            self.generate_steps_for_hunk(hunk);
            // Add pause between hunks
            self.steps.push(AnimationStep::Pause { duration_ms: 1500 });
        }
    }

    /// Generate animation steps for a diff hunk
    fn generate_steps_for_hunk(&mut self, hunk: &DiffHunk) {
        let mut current_old_line = hunk.old_start;
        let mut current_new_line = hunk.old_start;

        for line_change in &hunk.lines {
            match line_change.change_type {
                LineChangeType::Deletion => {
                    // Delete the entire line
                    self.steps.push(AnimationStep::MoveCursor {
                        line: current_old_line,
                        col: 0,
                    });
                    self.steps.push(AnimationStep::DeleteLine {
                        line: current_old_line,
                    });
                    self.steps.push(AnimationStep::Pause { duration_ms: 300 });
                    // Don't increment new_line for deletions
                }
                LineChangeType::Addition => {
                    // Insert empty line first
                    self.steps.push(AnimationStep::InsertLine {
                        line: current_new_line,
                        content: String::new(),
                    });

                    self.steps.push(AnimationStep::MoveCursor {
                        line: current_new_line,
                        col: 0,
                    });

                    // Type each character
                    let mut col = 0;
                    for ch in line_change.content.chars() {
                        self.steps.push(AnimationStep::InsertChar {
                            line: current_new_line,
                            col,
                            ch,
                        });
                        col += 1;
                    }

                    current_new_line += 1;
                    current_old_line += 1;
                    self.steps.push(AnimationStep::Pause { duration_ms: 200 });
                }
                LineChangeType::Context => {
                    // Just move cursor, no change
                    current_old_line += 1;
                    current_new_line += 1;
                }
            }
        }
    }

    /// Update animation state and return true if display needs refresh
    pub fn tick(&mut self) -> bool {
        // Handle cursor blinking
        if self.cursor_blink_timer.elapsed() >= Duration::from_millis(500) {
            self.cursor_visible = !self.cursor_visible;
            self.cursor_blink_timer = Instant::now();
        }

        // Check if we're paused
        if let Some(pause_until) = self.pause_until {
            if Instant::now() < pause_until {
                return true; // Keep blinking cursor during pause
            }
            self.pause_until = None;
        }

        if self.state != AnimationState::Playing {
            return false;
        }

        // Check if it's time for next step
        if self.last_update.elapsed() < Duration::from_millis(self.speed_ms) {
            return false;
        }

        // Execute next step
        if self.current_step >= self.steps.len() {
            self.state = AnimationState::Finished;
            return false;
        }

        let step = self.steps[self.current_step].clone();
        self.execute_step(step);
        self.current_step += 1;
        self.last_update = Instant::now();

        true
    }

    fn execute_step(&mut self, step: AnimationStep) {
        match step {
            AnimationStep::InsertChar { line, col, ch } => {
                self.buffer.insert_char(line, col, ch);
                self.buffer.cursor_line = line;
                self.buffer.cursor_col = col + 1;
            }
            AnimationStep::DeleteChar { line, col } => {
                self.buffer.delete_char(line, col);
                self.buffer.cursor_line = line;
                self.buffer.cursor_col = col;
            }
            AnimationStep::InsertLine { line, content } => {
                self.buffer.insert_line(line, content);
                self.buffer.cursor_line = line;
                self.buffer.cursor_col = 0;
            }
            AnimationStep::DeleteLine { line } => {
                self.buffer.delete_line(line);
                self.buffer.cursor_line = line;
                self.buffer.cursor_col = 0;
            }
            AnimationStep::MoveCursor { line, col } => {
                self.buffer.cursor_line = line;
                self.buffer.cursor_col = col;
            }
            AnimationStep::Pause { duration_ms } => {
                self.pause_until = Some(Instant::now() + Duration::from_millis(duration_ms));
            }
            AnimationStep::SwitchFile {
                file_index,
                content,
            } => {
                // Switch to new file
                self.current_file_index = file_index;
                self.buffer = EditorBuffer::from_content(&content);
            }
        }

        // Update scroll to keep cursor centered
        self.update_scroll();
    }

    fn update_scroll(&mut self) {
        if self.viewport_height == 0 {
            return;
        }

        let cursor_line = self.buffer.cursor_line;
        let total_lines = self.buffer.lines.len();
        let half_viewport = self.viewport_height / 2;

        // Try to center the cursor line
        let target_offset = if cursor_line < half_viewport {
            // Near the top of file, don't scroll
            0
        } else if cursor_line + half_viewport >= total_lines {
            // Near the bottom of file, show as much as possible
            total_lines.saturating_sub(self.viewport_height)
        } else {
            // Middle of file, center the cursor
            cursor_line.saturating_sub(half_viewport)
        };

        self.buffer.scroll_offset = target_offset;
    }

    pub fn is_finished(&self) -> bool {
        self.state == AnimationState::Finished
    }
}
