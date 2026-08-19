import { memo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { openUrl } from "@tauri-apps/plugin-opener";

import { cn } from "@/lib/cn";


export const Markdown = memo(function Markdown({
  source,
  className,
}: {
  source: string;
  className?: string;
}) {
  return (
    <div data-selectable className={cn("markdown", className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}


        rehypePlugins={[[rehypeHighlight, { ignoreMissing: true, detect: false }]]}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}


              onClick={(event) => {
                event.preventDefault();
                if (href && /^https?:/i.test(href)) void openUrl(href);
              }}
            >
              {children}
            </a>
          ),


          img: ({ alt, src }) => (
            <span className="markdown-image">{alt || String(src ?? "image")}</span>
          ),


          table: ({ children }) => (
            <div className="markdown-table">
              <table>{children}</table>
            </div>
          ),
        }}
      >
        {source}
      </ReactMarkdown>
    </div>
  );
});
