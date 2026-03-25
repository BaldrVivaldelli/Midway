import { useMemo } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { StreamLanguage } from "@codemirror/language";
import { EditorState, type Extension } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, placeholder as cmPlaceholder } from "@codemirror/view";
import { linter, lintGutter } from "@codemirror/lint";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { shell } from "@codemirror/legacy-modes/mode/shell";

export type CodeEditorLanguage = "json" | "text" | "shell";

type CodeEditorProps = {
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  language?: CodeEditorLanguage;
  minHeight?: number;
  placeholderText?: string;
  className?: string;
};

function languageExtension(language: CodeEditorLanguage): Extension | null {
  if (language === "json") {
    return [json(), linter(jsonParseLinter()), lintGutter()];
  }

  if (language === "shell") {
    return StreamLanguage.define(shell);
  }

  return null;
}

export default function CodeEditor({
  value,
  onChange,
  readOnly = false,
  language = "text",
  minHeight = 220,
  placeholderText = "",
  className
}: CodeEditorProps) {
  const extensions = useMemo(() => {
    const resolved = languageExtension(language);

    return [
      EditorState.readOnly.of(readOnly),
      EditorView.editable.of(!readOnly),
      EditorView.lineWrapping,
      cmPlaceholder(placeholderText),
      EditorView.theme({
        "&": {
          minHeight: `${minHeight}px`,
          backgroundColor: "#0b1117"
        },
        ".cm-scroller": {
          minHeight: `${minHeight}px`,
          fontFamily:
            '"SFMono-Regular", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
          fontSize: "13px",
          lineHeight: "1.6"
        },
        ".cm-content, .cm-gutter": {
          minHeight: `${minHeight}px`
        },
        ".cm-content": {
          padding: "14px 0"
        },
        ".cm-line": {
          padding: "0 16px"
        },
        ".cm-gutters": {
          backgroundColor: "#0b1117",
          color: "#738196",
          borderRight: "1px solid #1d2733",
          minHeight: `${minHeight}px`
        },
        ".cm-activeLine, .cm-activeLineGutter": {
          backgroundColor: readOnly ? "transparent" : "rgba(112, 127, 149, 0.08)"
        },
        ".cm-placeholder": {
          color: "#5c6b80"
        },
        ".cm-panels": {
          backgroundColor: "#11171f",
          color: "#eef2f8"
        },
        ".cm-tooltip": {
          backgroundColor: "#11171f",
          border: "1px solid #2a3644",
          color: "#eef2f8"
        },
        ".cm-tooltip-autocomplete ul li[aria-selected]": {
          backgroundColor: "#1b2330",
          color: "#ffffff"
        },
        ".cm-cursor": {
          borderLeftColor: "#f1f5fb"
        },
        ".cm-selectionBackground, ::selection": {
          backgroundColor: "rgba(104, 139, 255, 0.26) !important"
        },
        ".cm-diagnostic": {
          fontFamily:
            'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
        }
      }),
      oneDark,
      ...(resolved ? (Array.isArray(resolved) ? resolved : [resolved]) : [])
    ] satisfies Extension[];
  }, [language, minHeight, placeholderText, readOnly]);

  return (
    <div
      className={className ? `code-editor-shell ${className}` : "code-editor-shell"}
      data-language={language}
      style={{ minHeight }}
    >
      <CodeMirror
        aria-label="Editor de código"
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          highlightActiveLine: !readOnly,
          highlightActiveLineGutter: !readOnly,
          highlightSelectionMatches: true,
          closeBrackets: !readOnly,
          autocompletion: !readOnly,
          syntaxHighlighting: true,
          searchKeymap: true
        }}
        className="code-editor-root"
        editable={!readOnly}
        extensions={extensions}
        height="auto"
        onChange={(nextValue) => onChange?.(nextValue)}
        readOnly={readOnly}
        theme={oneDark}
        value={value}
      />
    </div>
  );
}
