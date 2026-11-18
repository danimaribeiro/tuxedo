//! Method calls, and the keywords shaped like them: `super`, `yield`,
//! `return`, `break` and `next`.
//!
//! A call is a receiver, a message, an argument list and maybe a block,
//! and the argument list follows the all-or-nothing rule of every
//! bracketed list (`call_args_doc` in `mod.rs`). The rule this file adds
//! is for method chains: a chain in which at least two links carry
//! arguments or a block is one group (`try_build_chain_doc`), so it either
//! fits on its line or breaks before every `.`/`&.`, each link one indent
//! in from the statement. A chain on an array or hash literal has a third
//! shape: the literal expands and the chain stays on its closing bracket's
//! line, taken whenever the chain fits there.

use super::doc::Doc;
use super::{Formatter, ItemGaps};

/// True for a call that can be a link in a `.method` chain: a plain
/// identifier-style call (not `[]`/`[]=`, not an attribute write, not a
/// `!`-style unary) with no real block of its own. A link with a block
/// joins a chain only through `block_can_join`.
fn is_plain_method_call(call: &ruby_prism::CallNode<'_>) -> bool {
    let name = call.name();
    let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");
    let is_index = name_str == "[]" || name_str == "[]=";
    let is_identifier_call = name_str
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_');
    is_identifier_call && !is_index && !call.is_attribute_write()
}

/// True for an index read in a chain (`doc.xpath(q)[i].text`): `[]` in
/// its bracket spelling, with arguments and nothing after them. It is a
/// link of the chain as `.fetch(i)` would be, and rides on the line of the
/// link before it, since Ruby reads a line that starts with `[` as a new
/// array.
fn is_index_link(call: &ruby_prism::CallNode<'_>) -> bool {
    call.name().as_slice() == b"[]"
        && call.call_operator_loc().is_none()
        && call.opening_loc().is_some()
        && call.arguments().is_some()
        && call.block().is_none()
}

/// True if `call`'s block can sit inside a chain link: a `{ }` block whose
/// body is at most one statement and holds nothing that lays out its own
/// lines, so it stays on its link's line. A `do...end` block, a body of
/// several statements, or a comment inside it ends the chain there: those
/// blocks are multi-line however much room there is, and a chain link that
/// is multi-line would force every link around it onto its own line.
fn block_can_join(
    call: &ruby_prism::CallNode<'_>,
    has_comment: &dyn Fn(usize, usize) -> bool,
) -> bool {
    let Some(block) = real_block(call) else {
        return true;
    };
    let Some(block) = block.as_block_node() else {
        return false;
    };
    if block.opening_loc().as_slice() != b"{" {
        return false;
    }
    if has_comment(
        block.opening_loc().end_offset(),
        block.closing_loc().start_offset(),
    ) {
        return false;
    }
    match block.body() {
        None => true,
        Some(body) => {
            let single = body
                .as_statements_node()
                .is_none_or(|s| s.body().iter().count() <= 1);
            single && !super::defs::holds_block_construct(&body)
        }
    }
}

/// Walks a call's receiver chain (`a.b.c.d` -> `d` is `node`) as far back as
/// it stays a plain method call (see `is_plain_method_call`), returning the
/// non-call (or otherwise unchainable) node the chain bottoms out at and
/// the links in between, in base-to-`node` order -- `node` itself is not
/// included, since the caller already holds it. `None` means there's
/// nothing to chain (the receiver isn't itself a chainable call), so the
/// caller should fall back to its normal single-call rendering.
fn collect_chain<'a>(
    node: &ruby_prism::CallNode<'a>,
    has_comment: &dyn Fn(usize, usize) -> bool,
) -> Option<(ruby_prism::Node<'a>, Vec<ruby_prism::CallNode<'a>>)> {
    let mut links = Vec::new();
    if is_scope_call(node) {
        return None;
    }
    let mut base = node.receiver()?;

    while let Some(call) = base.as_call_node() {
        let joins = is_index_link(&call)
            || (is_plain_method_call(&call)
                && block_can_join(&call, has_comment)
                && !is_scope_call(&call));
        if !joins {
            break;
        }
        match call.receiver() {
            Some(next_base) => {
                links.push(call);
                base = next_base;
            }
            None => break,
        }
    }

    let scoped_head = base
        .as_call_node()
        .is_some_and(|call| is_scope_call(&call) && is_plain_method_call(&call));
    if links.is_empty() && !scoped_head {
        None
    } else {
        links.reverse();
        Some((base, links))
    }
}

/// True for a call spelled with `::` (`Nokogiri::XML(doc)`). It cannot be a
/// chain link: a line that starts with `::` begins a new statement naming a
/// top-level constant, so breaking before it turns one call into two.
/// The chain stops at it instead, and it is the chain's first line. It
/// still counts as a link that carries work, so `Nokogiri::XML(doc).at(x)`
/// breaks the way `Nokogiri.XML(doc).at(x)` does.
fn is_scope_call(call: &ruby_prism::CallNode<'_>) -> bool {
    call.call_operator_loc()
        .is_some_and(|l| l.as_slice() == b"::")
}

/// The chain carried in `link`'s sole argument, when `link` is written
/// without parentheses: `.and change { B }.by(2)` hands back the base
/// `change { B }` and the links after it (`.by(2)`), base-to-end.
///
/// RSpec's compound matchers read as one chain (`.to change { A }.by(1)
/// .and change { B }.by(2)`), but Ruby parses each `change` as the argument
/// of the link before it. Laid out by the parse, each matcher would nest one
/// level deeper than the last; the chain takes these links in as its own
/// instead, at its one indent, which is what RuboCop's
/// `Layout/MultilineMethodCallIndentation` expects and what authors write.
///
/// A link with a block of its own keeps its argument: the block's header
/// belongs on the line the argument ends on.
fn carried_chain<'a>(
    link: &ruby_prism::CallNode<'a>,
    has_comment: &dyn Fn(usize, usize) -> bool,
) -> Option<(ruby_prism::Node<'a>, Vec<ruby_prism::CallNode<'a>>)> {
    if link.opening_loc().is_some() || link.block().is_some() || is_index_link(link) {
        return None;
    }
    let mut arguments = link.arguments()?.arguments().iter();
    let only = arguments.next()?;
    if arguments.next().is_some() {
        return None;
    }
    let last = only.as_call_node()?;
    let joins = |call: &ruby_prism::CallNode<'_>| {
        is_index_link(call)
            || (is_plain_method_call(call)
                && block_can_join(call, has_comment)
                && !is_scope_call(call))
    };
    if !joins(&last) {
        return None;
    }
    let mut links = vec![last];
    let mut base = links[0].receiver()?;
    while let Some(call) = base.as_call_node() {
        let Some(next_base) = call.receiver().filter(|_| joins(&call)) else {
            break;
        };
        links.push(call);
        base = next_base;
    }
    // Only a chain in its own right: two of its pieces carry arguments or a
    // block, as `change { B }.by(2)` does. `.find firm.id` and
    // `.and output(/x/).to_stdout` keep their argument whole; their `.id`
    // on a line of its own would read as a link of the outer chain.
    let base_works = base
        .as_call_node()
        .is_some_and(|call| call.arguments().is_some() || call.block().is_some());
    let working = links
        .iter()
        .filter(|call| {
            !is_index_link(call) && (call.arguments().is_some() || call.block().is_some())
        })
        .count()
        + usize::from(base_works);
    if working < 2 {
        return None;
    }
    links.reverse();
    Some((base, links))
}

/// A link that does more than read: it takes arguments or a block.
fn carries_work(call: &ruby_prism::CallNode<'_>) -> bool {
    !is_index_link(call) && (call.arguments().is_some() || call.block().is_some())
}

/// How many pieces of a chain carry work (`carries_work`): its links, its
/// last call, and a `::` head. A chain breaks only when two do; one with a
/// single working link stays a call on a receiver.
fn chain_work(
    base: &ruby_prism::Node<'_>,
    links: &[ruby_prism::CallNode<'_>],
    node: &ruby_prism::CallNode<'_>,
) -> usize {
    let scoped_head = base
        .as_call_node()
        .filter(|call| is_scope_call(call) && carries_work(call));
    links
        .iter()
        .chain(scoped_head.as_ref())
        .chain(std::iter::once(node))
        .filter(|c| carries_work(c))
        .count()
}

/// True for an operand that will lay out as a method chain
/// (`try_build_chain_doc`), seen through a leading `!`. In a block
/// condition it takes the condition's doubled indent, as an operand that
/// is an infix spine does: `Layout/MultilineMethodCallIndentation` holds a
/// chain's links in a condition two levels in from its receiver.
pub(super) fn is_condition_chain(node: &ruby_prism::Node<'_>) -> bool {
    let Some(mut call) = node.as_call_node() else {
        return false;
    };
    if call.name().as_slice() == b"!"
        && call.call_operator_loc().is_none()
        && call.arguments().is_none()
        && let Some(inner) = call.receiver().and_then(|r| r.as_call_node())
    {
        call = inner;
    }
    is_chain_operand(&call.as_node())
}

/// True for a node that lays out as a method chain (`try_build_chain_doc`):
/// two of its pieces carry work, so it breaks before every `.` when it
/// does not fit. Its last call is a `.` link or an index: an operator call
/// (`a.b(x) != c`) is an infix spine with the chain as an operand.
fn is_chain_operand(node: &ruby_prism::Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        (call.call_operator_loc().is_some() || is_index_link(&call))
            && collect_chain(&call, &|_, _| false)
                .is_some_and(|(base, links)| chain_work(&base, &links, &call) >= 2)
    })
}

/// Another handle on the same call: Prism's nodes are views into the
/// parse, and `CallNode` has no `Clone` of its own.
fn copy_call<'a>(call: &ruby_prism::CallNode<'a>) -> ruby_prism::CallNode<'a> {
    call.as_node()
        .as_call_node()
        .expect("a call node is a call node")
}

/// The `.`/`&.` a chain link is reached through, as written.
fn call_operator<'a>(call: &'a ruby_prism::CallNode<'a>) -> &'a str {
    call.call_operator_loc()
        .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
        .unwrap_or(".")
}

/// A call's text before its block, in two parts: everything up to the
/// argument list, and the list itself.
///
/// They are kept apart because the room a following ` do |params|` header
/// takes has to come off the argument list and nothing else. The receiver
/// has had its own line by the time the header lands, so holding room back
/// from it only makes a collection receiver expand to leave space it is not
/// competing for -- `[...].each do |camel, under, human, title|` broke its
/// array one element per line for a header 32 columns away.
struct CallHead {
    prefix: Doc,
    args: Doc,
}

impl CallHead {
    /// A call whose shape has no argument list to hold room back from.
    fn whole(doc: Doc) -> Self {
        CallHead {
            prefix: doc,
            args: Doc::nil(),
        }
    }
}

/// A value with brackets of its own to break inside: a call with a
/// parenthesized argument list, or an array or hash literal.
fn breaks_in_own_brackets(node: &ruby_prism::Node<'_>) -> bool {
    match node {
        ruby_prism::Node::CallNode { .. } => node.as_call_node().is_some_and(|call| {
            call.opening_loc()
                .is_some_and(|open| open.as_slice() == b"(")
                && call.arguments().is_some()
        }),
        ruby_prism::Node::ArrayNode { .. } => node
            .as_array_node()
            .and_then(|array| array.opening_loc())
            .is_some_and(|open| open.as_slice() == b"["),
        ruby_prism::Node::HashNode { .. } => true,
        _ => false,
    }
}

/// A collection literal with brackets of its own: an array in any
/// spelling (`[`, `%w[`, `%i(`) or a hash.
///
/// A call's argument list is not one. Expanding `receive(:readline)` so
/// that `.with(...).and_return(...)` can stay on the `)` line puts one
/// short argument on a line of its own; a call base breaks the links
/// instead, as a receiver's arguments do.
fn is_collection_literal(node: &ruby_prism::Node<'_>) -> bool {
    match node {
        ruby_prism::Node::ArrayNode { .. } => node
            .as_array_node()
            .is_some_and(|array| array.opening_loc().is_some()),
        ruby_prism::Node::HashNode { .. } => true,
        _ => false,
    }
}

/// `"a" "b"`, adjacent string literals Ruby joins into one string.
fn is_implicit_concat(node: &ruby_prism::Node<'_>) -> bool {
    node.as_interpolated_string_node()
        .is_some_and(|s| s.opening_loc().is_none() && s.parts().iter().count() > 1)
}

/// A call's real `{ }` / `do...end` block, as opposed to a block-pass
/// argument (`&:sym`), which Prism stores in the same slot but which
/// belongs inside the argument list.
fn real_block<'a>(node: &ruby_prism::CallNode<'a>) -> Option<ruby_prism::Node<'a>> {
    match node.block() {
        Some(block @ ruby_prism::Node::BlockNode { .. }) => Some(block),
        _ => None,
    }
}

impl Formatter {
    /// Renders `node` and its chainable receivers (see `collect_chain`) as
    /// one Doc group: either the whole chain fits on one line, or it breaks
    /// with a leading `.`/`&.` on every link, each at a fixed indent from
    /// the statement -- the same all-or-nothing, block-indented shape as an
    /// overlong array/call-args list, just applied to `.method` calls
    /// instead of comma-separated elements. Returns `None` when there's no
    /// real chain to break (fewer than two links carry arguments or a
    /// block), so the caller should fall back to its normal
    /// single-call path.
    ///
    /// Each link is a composed `Doc`, so an argument list of its own that
    /// has to wrap nests one level deeper than the link it belongs to --
    /// the indent a `Doc` carries is relative to the one already in effect,
    /// which here is the chain's own.
    ///
    /// `tail` is appended to the last link, inside the group: a block
    /// header shares the line that link's arguments end on, so it has to
    /// be measured as part of it rather than after the whole chain.
    fn try_build_chain_doc(&mut self, node: &ruby_prism::CallNode<'_>, tail: Doc) -> Option<Doc> {
        let (base, links) = {
            let this = &*self;
            collect_chain(node, &|start, end| this.has_pending_comment_in(start, end))?
        };

        // A chain of bare reads has nothing to break between but the dots,
        // and breaking there wrecks a line that is only long because of
        // what surrounds it. It stays one leaf, and the enclosing operator
        // or argument list takes the break. A comment between links has
        // to sit on a line of its own, so it still breaks them.
        // An index is not counted: `params[:user][:name].strip` is one
        // `.` link, with nothing to break between.
        let commented = links.iter().chain(std::iter::once(node)).any(|link| {
            !is_index_link(link)
                && link.receiver().is_some_and(|receiver| {
                    link.call_operator_loc()
                        .or_else(|| link.message_loc())
                        .is_some_and(|next| {
                            self.has_pending_comment_in(
                                receiver.location().end_offset(),
                                next.start_offset(),
                            )
                        })
                })
        });
        if chain_work(&base, &links, node) < 2 && !commented {
            return None;
        }

        // A chain that is the condition of a block `if`/`while` nests its
        // links two levels, so the condition cannot be mistaken for the
        // body that sits one level in (see `condition_doc`). Taken here,
        // after the chain is known to be one, so an argument list inside a
        // link never sees it.
        let levels = if std::mem::take(&mut self.condition_spine) {
            2
        } else {
            1
        };

        // The links in order, each with the base of the chain it carries
        // in place of its argument list (see `carried_chain`). Only the
        // chain's own last link hands its block to the caller as `tail`.
        let mut sequence: Vec<(ruby_prism::CallNode<'_>, Option<ruby_prism::Node<'_>>, bool)> =
            links
                .iter()
                .map(|link| (copy_call(link), None, true))
                .collect();
        let mut last = copy_call(node);
        let mut owns_tail = true;
        loop {
            let carried = {
                let this = &*self;
                carried_chain(&last, &|start, end| this.has_pending_comment_in(start, end))
            };
            let Some((carried_base, carried_links)) = carried else {
                break;
            };
            sequence.push((last, Some(carried_base), true));
            let (tail_link, rest) = carried_links
                .split_last()
                .expect("a carried chain has links");
            sequence.extend(rest.iter().map(|link| (copy_call(link), None, true)));
            last = copy_call(tail_link);
            owns_tail = false;
        }
        sequence.push((last, None, !owns_tail));

        let leading_index = links.first().is_some_and(is_index_link);
        let mut run = self.begin_run();
        self.run_node(&mut run, &base);
        // Every link sits one indent level deeper than the statement once
        // the chain breaks. The `Doc`s count their indent from the chain's,
        // so the printer handles the layout -- but a comment that has to be
        // put back one level (see `comments_doc`) reads `self.indent`, so
        // the counter is kept in step with the nesting.
        self.indent += levels;
        // Built link by link: the comments between one link and the next
        // are claimed after the first link's arguments and before the
        // second's.
        let mut segments: Vec<(Option<Doc>, Doc)> = Vec::new();
        for (link, carried_base, carries_block) in &sequence {
            if is_index_link(link) {
                let index = self.index_link_doc(link);
                match segments.last_mut() {
                    Some((_, last)) => *last = last.clone().append(index),
                    None => run.leaves.push(index),
                }
                continue;
            }
            let gap = self.link_gap_comments_doc(link);
            let args = match carried_base {
                Some(base) => Doc::text(" ").append(self.node_doc(base)),
                None => self.call_args_doc(link, link.opening_loc().is_some()),
            };
            let mut segment = Doc::text(call_operator(link))
                .append(super::bytes_doc(link.name().as_slice()))
                .append(args);
            // The last link's block is the caller's: its header is `tail`
            // and its body follows the chain. Any other link carries its
            // block along.
            if *carries_block && let Some(block) = real_block(link).and_then(|b| b.as_block_node())
            {
                let header = self.block_header_doc(&block);
                let body = self.block_body_doc(&block);
                segment = segment.append(header).append(body);
            }
            segments.push((gap, segment));
        }
        self.indent -= levels;
        if let Some((_, last)) = segments.last_mut() {
            *last = last.clone().append(tail);
        }
        let base_doc = self.end_run(run);

        // A comment between two links has no flat shape: every link goes on
        // its own line, as if the chain were too long.
        let commented = segments.iter().any(|(gap, _)| gap.is_some());
        let nest_width = self.indent_str.len() * levels;
        let links_doc = Doc::concat(segments.into_iter().map(|(gap, segment)| {
            let line = if commented {
                Doc::hardline()
            } else {
                Doc::softline()
            };
            gap.unwrap_or_else(Doc::nil).append(line).append(segment)
        }));
        let links_doc = links_doc.indent(nest_width);
        // A base that is multi-line at any width (`end.keys` after an `if`,
        // a `do...end` block) has no shape to flip between, and the links
        // go on its last line while they fit there.
        if base_doc.breaks() {
            return Some(base_doc.append(links_doc.group()));
        }
        // Otherwise the base is inside the group, so the question is
        // whether the whole chain fits, not whether what follows the base
        // does: a base that has to expand breaks the links as a receiver's
        // arguments would (`Set.new(...).map { }.join`).
        let broken = base_doc.clone().append(links_doc.clone()).group();
        // A collection literal (`[...]`, `%w[...]`, `{...}`) can expand
        // instead, leaving the chain on the closer's line. The closer goes
        // back to the statement's indent however long the elements are, so
        // whether the chain fits there is decided by the chain alone, and
        // lengthening an element cannot flip the links between the two
        // shapes.
        let closer_base = is_collection_literal(&base) && !leading_index && !links_doc.breaks();
        if closer_base {
            // Flat text rather than a group, so that the base's own group
            // measures the chain with it and expands when the two do not
            // fit on one line, instead of staying flat and pushing the
            // chain onto lines of its own.
            let links = links_doc.clone().group().flat();
            // Every closer (`]`, `)`, `}`) is one column wide.
            let tail = format!("]{}", links.render(0));
            return Some(Doc::fits_at_indent_or(
                &tail,
                base_doc.append(links),
                broken,
            ));
        }
        Some(broken)
    }

    /// The `[args]` of an index link, expanded like any bracketed list when
    /// it is too long.
    fn index_link_doc(&mut self, link: &ruby_prism::CallNode<'_>) -> Doc {
        let args: Vec<_> = link
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let span = link
            .opening_loc()
            .zip(link.closing_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        self.bracketed_items_doc(
            "[",
            Doc::softline(),
            &args,
            "]",
            span,
            ItemGaps::BetweenKeywordPairs,
        )
    }

    /// The comments between `link`'s receiver and the `.` that reaches
    /// `link`: the document that ends the previous line, ahead of the
    /// line break the link starts after. `None` when there are none.
    fn link_gap_comments_doc(&mut self, link: &ruby_prism::CallNode<'_>) -> Option<Doc> {
        let receiver = link.receiver()?;
        let next_start = link
            .call_operator_loc()
            .or_else(|| link.message_loc())?
            .start_offset();
        self.gap_comments_doc(receiver.location().end_offset(), next_start)
    }

    /// A block's ` do |params|` / ` { |params|` header, as a `Doc` to
    /// append to the argument list it will share a line with. Nothing when
    /// the call has no block, or carries a block-pass argument (`&:sym`)
    /// in that slot instead.
    fn block_tail_doc(&mut self, block: Option<&ruby_prism::Node<'_>>) -> Doc {
        match block.and_then(|b| b.as_block_node()) {
            Some(block) => self.block_header_doc(&block),
            None => Doc::nil(),
        }
    }

    pub(super) fn fmt_call_node(&mut self, node: &ruby_prism::CallNode<'_>) -> Doc {
        // ` do |x|` shares the line the argument list ends on, so it is
        // part of what has to fit. Threaded into the head, so whichever
        // group ends up holding the arguments -- a plain list, or the last
        // link of a chain -- measures the header along with them.
        let block = real_block(node);
        let tail = self.block_tail_doc(block.as_ref());
        let head = self.call_head_doc(node, tail);

        match block {
            Some(block) => self.with_block_doc(head, &block),
            None => head.prefix.append(head.args),
        }
    }

    /// A call without its block: the receiver, the message and the
    /// arguments, in whichever of Ruby's spellings the call uses.
    fn call_head_doc(&mut self, node: &ruby_prism::CallNode<'_>, tail: Doc) -> CallHead {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");

        let Some(receiver) = node.receiver() else {
            let mut run = self.begin_run();
            self.run_text(&mut run, name_str);
            let args = self.call_args_doc(node, node.opening_loc().is_some());
            let prefix = self.end_run(run);
            return CallHead {
                prefix,
                args: args.append(tail),
            };
        };

        // An operator method called with an explicit `.`/`&.`
        // (`a&.+(1)`, `env&.[]("k")`, `x&.!= y`) is written as an
        // ordinary method call, and has to stay one: the sugar forms
        // below (`a + 1`, `env["k"]`) have no way to spell `&.`.
        let explicit_dot = node.call_operator_loc().is_some();
        let is_index = (name_str == "[]" || name_str == "[]=") && !explicit_dot;
        let is_named_call = name_str
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let is_identifier_call = is_named_call || explicit_dot;

        if is_index {
            return CallHead {
                prefix: self.index_call_doc(node, &receiver, name_str),
                args: tail,
            };
        }
        if matches!(name_str, "!" | "~" | "+@" | "-@")
            && node.arguments().is_none()
            && !explicit_dot
        {
            return CallHead::whole(self.unary_call_doc(node, &receiver, name_str));
        }
        if is_named_call && node.is_attribute_write() {
            return CallHead::whole(self.attribute_write_doc(node, &receiver, name_str));
        }
        if is_identifier_call {
            if let Some(doc) = self.try_build_chain_doc(node, tail.clone()) {
                return CallHead {
                    prefix: Doc::nil(),
                    args: doc,
                };
            }
            let mut run = self.begin_run();
            self.run_node(&mut run, &receiver);
            // A comment before the link puts it on a line of its own, under
            // the comment and one level in.
            let gap = self.link_gap_comments_doc(node);
            let commented = gap.is_some();
            let mut link = gap.map_or_else(Doc::nil, |gap| gap.append(Doc::hardline()));
            if commented {
                self.indent += 1;
            }
            link = link
                .append(Doc::text(call_operator(node)))
                .append(Doc::text(name_str));
            let args = self.call_args_doc(node, node.opening_loc().is_some());
            if commented {
                self.indent -= 1;
                link = link.indent(self.indent_str.len());
            }
            let prefix = self.end_run(run).append(link);
            return CallHead {
                prefix,
                args: args.append(tail),
            };
        }
        CallHead::whole(self.operator_call_doc(node, receiver, name_str))
    }

    /// `recv[a, b]` and `recv[a] = value` -- the `[]`/`[]=` methods in
    /// their bracket spelling.
    fn index_call_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: &ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        let mut args: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let value = if name_str == "[]=" { args.pop() } else { None };

        let mut run = self.begin_run();
        self.run_node(&mut run, receiver);
        // `Dir["a", "b", ...]` reads as a list, and expands like one when it
        // is too long.
        let block_pass = node
            .block()
            .filter(|b| b.as_block_argument_node().is_some());
        if name_str == "[]"
            && !args.is_empty()
            && block_pass.is_none()
            && !self.single_line
            && let (Some(open), Some(close)) = (node.opening_loc(), node.closing_loc())
        {
            let span = Some((open.end_offset(), close.start_offset()));
            let list = self.bracketed_items_doc(
                "[",
                Doc::softline(),
                &args,
                "]",
                span,
                ItemGaps::BetweenKeywordPairs,
            );
            run.leaves.push(list);
            return self.end_run(run);
        }
        self.run_text(&mut run, "[");
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, arg);
        }
        if let Some(block_pass) = node
            .block()
            .filter(|b| b.as_block_argument_node().is_some())
        {
            if !args.is_empty() {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, &block_pass);
        }
        self.run_text(&mut run, "]");
        if let Some(value) = value {
            self.run_assigned_value(&mut run, "=", &value);
        }
        self.end_run(run)
    }

    /// A unary operator method (defined as `def +@`/`def -@`/`def ~`/
    /// `def !`) is called in prefix notation (`+x`), unlike every other
    /// operator method here, which is infix (`a + b`).
    /// `operator_call_doc` assumes infix and would render this
    /// receiver-first (`x +@`, dropping the fact there's no actual
    /// right-hand side) -- invalid Ruby.
    fn unary_call_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: &ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        // `not x` is the `!` method too, but binds far looser: `not a == b`
        // is `!(a == b)` while `!a == b` is `(!a) == b`. Keep the keyword,
        // and its parentheses.
        let is_not_keyword = node
            .message_loc()
            .is_some_and(|loc| loc.as_slice() == b"not");
        let not_parenthesized = is_not_keyword && node.opening_loc().is_some();

        let mut run = self.begin_run();
        if is_not_keyword {
            self.run_text(&mut run, if not_parenthesized { "not(" } else { "not " });
        } else {
            self.run_text(&mut run, name_str.strip_suffix('@').unwrap_or(name_str));
        }
        // `+ 1.second` calls `+@` on `1.second`; `+1.second` calls
        // `second` on the literal `+1`. Only a space keeps a sign from
        // fusing with a numeric literal that follows it.
        let receiver_starts_with_digit = self
            .source
            .as_bytes()
            .get(receiver.location().start_offset())
            .is_some_and(u8::is_ascii_digit);
        if matches!(name_str, "+@" | "-@") && receiver_starts_with_digit {
            self.run_text(&mut run, " ");
        }
        self.run_node(&mut run, receiver);
        if not_parenthesized {
            self.run_text(&mut run, ")");
        }
        self.end_run(run)
    }

    /// `recv.attr = value` -- the `attr=` method in its assignment
    /// spelling.
    fn attribute_write_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: &ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, receiver);
        self.run_text(&mut run, call_operator(node));
        self.run_text(&mut run, name_str.strip_suffix('=').unwrap_or(name_str));
        match node.arguments().and_then(|a| a.arguments().iter().next()) {
            Some(value) => self.run_assigned_value(&mut run, "=", &value),
            None => self.run_text(&mut run, " = "),
        }
        self.end_run(run)
    }

    /// Operator-style call (`a + b`, `a <=> b`, ...) as one all-or-nothing
    /// group, so an overlong line breaks after each operator with the
    /// operands indented one level instead of rendering flat regardless of
    /// width. `a + b + c` is nested `CallNode`s, and one group per node
    /// bin-packs, so the same-operator spine is flattened into one group
    /// first -- the same shape, and the same helper, as `&&`/`||`.
    ///
    /// Falls back to the flat shape when an operand lays out its own
    /// multi-line body (`if`/`case`/`begin`/a `do...end` call): no line
    /// point of this group could move it.
    fn operator_call_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        let single_arg = node
            .arguments()
            .map(|a| a.arguments().iter().collect::<Vec<_>>())
            .filter(|args| args.len() == 1)
            .map(|mut args| args.remove(0));

        let Some(arg) = single_arg else {
            let mut run = self.begin_run();
            self.run_node(&mut run, &receiver);
            self.run_text(&mut run, " ");
            self.run_text(&mut run, name_str);
            if let Some(args) = node.arguments() {
                self.run_text(&mut run, " ");
                for (i, arg) in args.arguments().iter().enumerate() {
                    if i > 0 {
                        self.run_text(&mut run, ", ");
                    }
                    self.run_node(&mut run, &arg);
                }
            }
            return self.end_run(run);
        };

        // Exponentiation is the one operator written tight (`2**8`), and it
        // has no line point: an overlong operand breaks inside itself.
        // `**` is right-associative, so there is no left spine to flatten.
        if name_str == "**"
            && !self.has_pending_comment_in(
                receiver.location().end_offset(),
                arg.location().start_offset(),
            )
        {
            let mut run = self.begin_run();
            self.run_node(&mut run, &receiver);
            self.run_text(&mut run, "**");
            self.run_node(&mut run, &arg);
            return self.end_run(run);
        }

        let mut operands = Vec::new();
        Self::operator_operands(node, name_str, &mut operands);
        operands.push(arg);

        let condition = std::mem::take(&mut self.condition_spine);

        // `errors << PolicyError.new(...)` appends one value the way
        // `error = PolicyError.new(...)` assigns one, and is laid out the
        // same way: `errors << PolicyError.new(` stays on the line and the
        // value breaks inside its own brackets, moving under the `<<` only
        // when even that head does not fit. Broken after the operator
        // instead, the statement would change shape whenever its argument
        // happened to span lines.
        if name_str == "<<"
            && !condition
            && let [target, value] = operands.as_slice()
            && breaks_in_own_brackets(value)
            && !self.has_pending_comment_in(
                target.location().end_offset(),
                value.location().start_offset(),
            )
        {
            let mut run = self.begin_run();
            self.run_node(&mut run, target);
            self.run_assigned_value(&mut run, "<<", value);
            return self.end_run(run);
        }

        let (leaves, gaps) = self.operand_docs(&operands, condition);

        // `"a %<x>s " \` + newline + `"b" % { x: 1 }`: the operator ends the
        // continuation line of the string, one level in, so what follows it
        // continues from there. Left at the statement's level, an expanded
        // hash would close left of the line that opened it.
        if let [string, _] = operands.as_slice()
            && is_implicit_concat(string)
            && gaps.iter().all(Option::is_none)
            && leaves[0].breaks()
        {
            let mut leaves = leaves.into_iter();
            let string = leaves.next().unwrap_or_else(Doc::nil);
            let value = leaves.next().unwrap_or_else(Doc::nil);
            return string.append(
                Doc::text(format!(" {name_str} "))
                    .append(value)
                    .indent(self.indent_str.len()),
            );
        }

        // `Post.where(...).limit(n).count > max`: a method chain that has to
        // break anyway ends on a line of its own, and the comparison stays
        // on it. Broken after the operator as well, the right side would
        // dangle on a line by itself under the chain's last link. While
        // the chain fits on the line with its operator, the spine breaks
        // the usual way.
        if let [chain, _] = operands.as_slice()
            && is_chain_operand(chain)
            && gaps.iter().all(Option::is_none)
            && !leaves.iter().any(Doc::breaks)
        {
            let head = format!(
                "{} {name_str}",
                leaves[0].clone().flat().render(usize::MAX / 2)
            );
            let sticky = leaves[0]
                .clone()
                .append(Doc::text(format!(" {name_str} ")))
                .append(leaves[1].clone());
            let spine = self.infix_doc(leaves, gaps, name_str, condition, false);
            return Doc::stay_or_move(&head, spine, sticky);
        }
        self.infix_doc(leaves, gaps, name_str, condition, false)
    }

    /// The operands of `node`'s receiver spine, in source order, as long as
    /// it stays the same operator written the same way (`a + b + c` is
    /// `+(+(a, b), c)`). Anything else -- a different message, an explicit
    /// `.`, a block, more or fewer than one argument -- ends the spine and
    /// becomes an operand of its own.
    fn operator_operands<'a>(
        node: &ruby_prism::CallNode<'a>,
        name_str: &str,
        out: &mut Vec<ruby_prism::Node<'a>>,
    ) {
        let Some(receiver) = node.receiver() else {
            return;
        };
        let inner = receiver.as_call_node().filter(|call| {
            call.call_operator_loc().is_none()
                && call.block().is_none()
                && call.receiver().is_some()
                && call.name().as_slice() == name_str.as_bytes()
                && call
                    .arguments()
                    .is_some_and(|a| a.arguments().iter().count() == 1)
        });
        match inner {
            Some(inner) => {
                Self::operator_operands(&inner, name_str, out);
                if let Some(arg) = inner.arguments().and_then(|a| a.arguments().iter().next()) {
                    out.push(arg);
                }
            }
            None => out.push(receiver),
        }
    }

    /// `head` with a real `{ }` / `do...end` block appended.
    ///
    /// The block's *header* is not appended here: `head.args` already
    /// carries it (see `fmt_call_node`), so the argument list and the
    /// ` do |x|` that shares its last line render as one document and the
    /// printer measures the line it will really produce.
    fn with_block_doc(&mut self, head: CallHead, block: &ruby_prism::Node<'_>) -> Doc {
        let body = match block.as_block_node() {
            Some(b) => self.block_body_doc(&b),
            None => self.node_doc(block),
        };
        head.prefix.append(head.args).append(body)
    }

    pub(super) fn fmt_super_node(&mut self, node: &ruby_prism::SuperNode<'_>) -> Doc {
        let positional: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let block_pass = match node.block() {
            Some(b @ ruby_prism::Node::BlockArgumentNode { .. }) => Some(b),
            _ => None,
        };
        let block = match node.block() {
            Some(block @ ruby_prism::Node::BlockNode { .. }) => Some(block),
            _ => None,
        };
        let span = node
            .lparen_loc()
            .zip(node.rparen_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));

        let tail = self.block_tail_doc(block.as_ref());
        let mut run = self.begin_run();
        self.run_text(&mut run, "super");
        let args = self.arg_list_doc(node.lparen_loc().is_some(), positional, block_pass, span);
        let head = CallHead {
            prefix: self.end_run(run),
            args: args.append(tail),
        };

        match block {
            Some(block) => self.with_block_doc(head, &block),
            None => head.prefix.append(head.args),
        }
    }

    /// `...` as a call argument, forwarding a `...` parameter through.
    pub(super) fn fmt_forwarding_arguments_node(
        &mut self,
        _node: &ruby_prism::ForwardingArgumentsNode<'_>,
    ) -> Doc {
        Doc::text("...")
    }

    pub(super) fn fmt_forwarding_super_node(
        &mut self,
        node: &ruby_prism::ForwardingSuperNode<'_>,
    ) -> Doc {
        let Some(block) = node.block() else {
            return Doc::text("super");
        };
        let block = self.fmt_block_node(&block);
        Doc::text("super").append(block)
    }

    pub(super) fn fmt_yield_node(&mut self, node: &ruby_prism::YieldNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "yield");
        let Some(args) = node.arguments() else {
            return self.end_run(run);
        };
        // Parens around yield's arguments are load-bearing, not decorative:
        // `cond ? yield(x) : y` is valid Ruby while `cond ? yield x : y` is
        // a syntax error. Keep whichever form the source used.
        let span = node
            .lparen_loc()
            .zip(node.rparen_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        let doc = self.arg_list_doc(
            node.lparen_loc().is_some(),
            args.arguments().iter().collect(),
            None,
            span,
        );
        let prefix = self.end_run(run);
        prefix.append(doc)
    }

    pub(super) fn fmt_return_node(&mut self, node: &ruby_prism::ReturnNode<'_>) -> Doc {
        self.jump_doc("return", node.arguments())
    }

    pub(super) fn fmt_break_node(&mut self, node: &ruby_prism::BreakNode<'_>) -> Doc {
        self.jump_doc("break", node.arguments())
    }

    pub(super) fn fmt_next_node(&mut self, node: &ruby_prism::NextNode<'_>) -> Doc {
        self.jump_doc("next", node.arguments())
    }

    /// `return`/`break`/`next`, with their values comma-joined on the same
    /// line. No line point: a jump with more than one value is rare enough
    /// that a wrapped shape has never been needed, and `arg_list_doc`'s is
    /// not it -- these take no parentheses.
    ///
    /// A single `return` value that wraps (`return a ||\n b`, `return
    /// Loader\n .new(...)`) continues two levels in, the way a block
    /// condition does: `Layout/MultilineOperationIndentation` and
    /// `Layout/MultilineMethodCallIndentation` give `return` the same
    /// doubled indent as `if`/`while`, and `break`/`next` the ordinary one.
    fn jump_doc(&mut self, keyword: &str, arguments: Option<ruby_prism::ArgumentsNode<'_>>) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, keyword);
        if let Some(arguments) = arguments {
            let single = arguments.arguments().iter().count() == 1;
            self.run_text(&mut run, " ");
            for (i, arg) in arguments.arguments().iter().enumerate() {
                if i > 0 {
                    self.run_text(&mut run, ", ");
                }
                self.condition_spine = keyword == "return" && single;
                self.run_node(&mut run, &arg);
                self.condition_spine = false;
            }
        }
        self.end_run(run)
    }
}
