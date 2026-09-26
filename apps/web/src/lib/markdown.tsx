// Renders the small subset of Markdown that RepoDNA's own documents use (the privacy policy
// and the terms of use): headings, paragraphs, bulleted lists, links, code, and bold text.
// The documents are bundled at build time; everything becomes React elements, never HTML.

import type { ReactNode } from "react";

export type Block =
  | { kind: "heading"; level: 1 | 2 | 3; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; items: string[] };

/** Splits a document into headings, paragraphs, and lists. */
export function parseMarkdown(source: string): Block[] {
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  let list: string[] | null = null;
  const endParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
  };
  const endList = () => {
    if (list) {
      blocks.push({ kind: "list", items: list });
      list = null;
    }
  };
  for (const raw of source.replace(/\r\n/g, "\n").split("\n")) {
    const line = raw.trimEnd();
    const heading = /^(#{1,3}) (.+)$/.exec(line);
    if (line.trim() === "") {
      endParagraph();
      endList();
    } else if (heading?.[1] && heading[2]) {
      endParagraph();
      endList();
      blocks.push({
        kind: "heading",
        level: heading[1].length as 1 | 2 | 3,
        text: heading[2].trim(),
      });
    } else if (line.startsWith("- ")) {
      endParagraph();
      list ??= [];
      list.push(line.slice(2).trim());
    } else if (list && /^\s/.test(line)) {
      // A continuation line of the current list item.
      const items: string[] = list;
      items[items.length - 1] += ` ${line.trim()}`;
    } else {
      endList();
      paragraph.push(line.trim());
    }
  }
  endParagraph();
  endList();
  return blocks;
}

/** Makes a link element; the caller decides how links open. */
export type LinkRenderer = (url: string, children: ReactNode, key: string) => ReactNode;

const INLINE = /\[([^\]]+)\]\(([^)\s]+)\)|<(https?:\/\/[^>\s]+)>|`([^`]+)`|\*\*([^*]+)\*\*/g;

/** Turns links, code, and bold text in one line into elements. */
export function renderInline(text: string, link: LinkRenderer, prefix = ""): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let index = 0;
  for (const match of text.matchAll(INLINE)) {
    const [whole, label, url, autolink, code, bold] = match;
    if (match.index > last) {
      out.push(text.slice(last, match.index));
    }
    const key = `${prefix}${index++}`;
    if (label !== undefined && url !== undefined) {
      out.push(link(url, renderInline(label, link, `${key}.`), key));
    } else if (autolink !== undefined) {
      out.push(link(autolink, autolink, key));
    } else if (code !== undefined) {
      out.push(<code key={key}>{code}</code>);
    } else if (bold !== undefined) {
      out.push(<strong key={key}>{bold}</strong>);
    }
    last = match.index + whole.length;
  }
  if (last < text.length) {
    out.push(text.slice(last));
  }
  return out;
}

/** A document rendered from Markdown. */
export function MarkdownDocument({ source, link }: { source: string; link: LinkRenderer }) {
  return (
    <article className="document">
      {parseMarkdown(source).map((block, index) => {
        const key = String(index);
        if (block.kind === "heading") {
          const Heading = `h${block.level}` as const;
          return <Heading key={key}>{renderInline(block.text, link, `${key}.`)}</Heading>;
        }
        if (block.kind === "list") {
          return (
            <ul key={key}>
              {block.items.map((item, itemIndex) => (
                <li key={itemIndex}>{renderInline(item, link, `${key}.${itemIndex}.`)}</li>
              ))}
            </ul>
          );
        }
        return <p key={key}>{renderInline(block.text, link, `${key}.`)}</p>;
      })}
    </article>
  );
}
