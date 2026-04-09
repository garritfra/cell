use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use cell_core::help::HelpRegistry;

pub struct HelpView<'a> {
    pub registry: &'a HelpRegistry,
    pub topic: Option<&'a str>,
    pub scroll: usize,
}

impl<'a> HelpView<'a> {
    /// Render the table of contents into lines.
    fn render_toc(&self) -> Vec<String> {
        let mut lines = vec![
            String::new(),
            "Cell — Terminal Spreadsheet Editor".to_string(),
            String::new(),
            "Use :help <topic> for details on any entry below.".to_string(),
            String::new(),
        ];

        for category in self.registry.categories() {
            lines.push(String::new());
            lines.push(category.label().to_string());
            lines.push(String::new());

            for entry in self.registry.by_category(category) {
                let tag = entry.tags[0];
                let padding = 16usize.saturating_sub(tag.len());
                lines.push(format!("  {}{}{}", tag, " ".repeat(padding), entry.summary));
            }
        }

        lines.push(String::new());
        lines
    }

    /// Render a specific topic's detail view into lines.
    fn render_topic(&self, tag: &str) -> Vec<String> {
        let mut lines = Vec::new();

        if let Some(entry) = self.registry.find(tag) {
            lines.push(String::new());

            // Title: all tags for this entry
            let all_tags: Vec<&str> = entry.tags.to_vec();
            lines.push(all_tags.join(", "));
            lines.push(String::new());

            // Summary as a heading
            lines.push(entry.summary.to_string());
            lines.push(String::new());

            // Detail text
            for line in entry.detail.lines() {
                lines.push(line.to_string());
            }

            lines.push(String::new());
            lines.push(format!("Category: {}", entry.category.label()));
            lines.push(String::new());
        }

        lines
    }

    fn content_lines(&self) -> Vec<String> {
        match self.topic {
            Some(tag) => self.render_topic(tag),
            None => self.render_toc(),
        }
    }
}

impl<'a> Widget for HelpView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 {
            return;
        }

        let title_area = Rect { height: 1, ..area };
        let content_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(2),
            ..area
        };
        let footer_area = Rect {
            y: area.y + area.height.saturating_sub(1),
            height: 1,
            ..area
        };

        // Title bar
        let title_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let title_text = match self.topic {
            Some(tag) => format!(" Cell — Help: {}", tag),
            None => " Cell — Help".to_string(),
        };
        for x in title_area.x..title_area.x + title_area.width {
            buf.set_string(x, title_area.y, " ", title_style);
        }
        buf.set_string(title_area.x, title_area.y, &title_text, title_style);

        // Content
        let lines = self.content_lines();
        let visible_height = content_area.height as usize;
        let max_scroll = lines.len().saturating_sub(visible_height);
        let scroll = self.scroll.min(max_scroll);
        for (i, line) in lines.iter().skip(scroll).take(visible_height).enumerate() {
            let y = content_area.y + i as u16;
            let truncated: String = line.chars().take(content_area.width as usize).collect();
            buf.set_string(content_area.x + 1, y, &truncated, Style::default());
        }

        // Footer
        let footer_style = Style::default().fg(Color::Black).bg(Color::White);
        for x in footer_area.x..footer_area.x + footer_area.width {
            buf.set_string(x, footer_area.y, " ", footer_style);
        }
        let footer_text = " Press q to return │ j/k scroll │ :help <topic>";
        buf.set_string(footer_area.x, footer_area.y, footer_text, footer_style);
    }
}
