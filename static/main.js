const ws = new WebSocket("/ws");
let data = null;
let updateGitShowTimer = null;
ws.addEventListener("message", event => {
    console.log(event.data);
    data = JSON.parse(event.data);
    title.className = data.commit.state;
    title.textContent = data.commit.title;
    meta.textContent = `${data.commit.date} – ${data.commit.authors.join(", ")}`;
    label.textContent = data.commit.label;
    hints.innerHTML = "";
    for (const hint of data.commit.hints) {
        const li = document.createElement("li");
        li.append(hint);
        hints.append(li);
    }
    content.innerHTML = data.rendered_body;
    input.value = data.commit.label;

    // linkify any text that looks like a qualified issue reference. to avoid
    // false positives, only consider direct descendant text nodes of <p>.
    linkify(
        content.querySelectorAll("p"), /([0-9A-Za-z-]+)[/]([^ ]+)#([0-9]+)/gd,
        (linkText, owner, repo, number) => `https://github.com/${owner}/${repo}/issues/${number}`,
    );

    // linkify any text that looks like github issue references. but to avoid
    // false positives, only consider direct descendant text nodes of <p>,
    // plus the commit title line.
    linkify(
        [title, ...content.querySelectorAll("p")], /#[0-9]+/gd,
        linkText => `https://github.com/servo/servo/issues/${linkText.slice(1)}`,
    );

    // linkify any text that looks like github user mentions. but to avoid
    // false positives, only consider direct descendant text nodes of <p>,
    // plus the commit authors line.
    linkify(
        [meta, ...content.querySelectorAll("p")], /@[0-9A-Za-z-]+/gd,
        linkText => `https://github.com/${linkText.slice(1)}`,
    );

    // linkify any text that looks like git commit hashes. but to avoid
    // false positives, only consider direct descendant text nodes of <p>.
    linkify(
        content.querySelectorAll("p"), /\b[0-9a-f]{7,}\b/gd,
        linkText => `https://github.com/servo/servo/commit/${linkText}`,
    );

    // make all links open in a separate tab by default.
    for (const a of document.querySelectorAll(":any-link")) {
        // like `_blank`, but opens in the same tab every time, so you can keep
        // this tool and the links you click in two separate windows.
        // if you want to open in a new tab, you can still use Ctrl+click or
        // middle click. likewise for a new window, you can still Shift+click.
        a.target = "anotherWindow";
    }

    // after some delay, update the `git show` output.
    // rendering the `git show` output can be expensive.
    if (updateGitShowTimer != null) {
        clearTimeout(updateGitShowTimer);
        updateGitShowTimer = null;
    }
    updateGitShowTimer = setTimeout(updateGitShow, 50);
});
addEventListener("keypress", event => {
    console.log(event);
    if (editorDialog.open) {
        return;
    }
    switch (event.key) {
    case "q":
        // require the user to focus the commit-triage TUI to quit.
        // even though this page is in focus, you may still be able to scroll
        // through `git show` in a nearby terminal. if you then press `Q` to
        // quit the `git show` pager without focusing the terminal, you would
        // quit the commit-triage tool by mistake. so we prevent that.
        break;
    case "t":
        editorDialog.showModal();
        break;
    default:
        ws.send(JSON.stringify({"Keypress": event.key}));
    }
});
// when you Shift+scroll, step through commits.
// unlike Ctrl+scroll, Alt+scroll, and scrolling without modifiers, this does
// not seem to interfere with typical browser default behaviour.
addEventListener("wheel", event => {
    console.log(event);
    if (event.target.id == "gitShow") {
        return;
    }
    if (!event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) {
        return;
    }
    if (event.deltaY > 0) {
        ws.send(JSON.stringify({"Keypress": "j"}));
        return;
    }
    if (event.deltaY < 0) {
        ws.send(JSON.stringify({"Keypress": "k"}));
        return;
    }
});
editorDialog.addEventListener("close", event => {
    console.log(event);
    ws.send(JSON.stringify({"Reload": null}));
});
editorForm.addEventListener("submit", event => {
    console.log(event);
    ws.send(JSON.stringify({"SetLabel": input.value}));
});
function linkify(parents, regex, hrefFn) {
    for (const parent of parents) {
        for (const kid of parent.childNodes) {
            if (kid.nodeName != "#text")
                continue;
            const originalText = kid.nodeValue;
            // array of matches
            // where each match is an array [matchedText, ...captureGroups]
            // where each element is a pair of indices [start, stop]
            const matches = [];
            let result;
            while ((result = regex.exec(originalText)) != null) {
                matches.push(result.indices);
            }
            matches.reverse();
            for (const match of matches) {
                const [matchedText, ...captureGroups] = match;
                const [start, stop] = matchedText;
                kid.splitText(stop);
                const text = kid.splitText(start);
                const a = document.createElement("a");
                a.href = hrefFn(...match.map(([start, stop]) => originalText.slice(start, stop)));
                text.replaceWith(a);
                a.append(text);
            }
        }
    }
}
function updateGitShow() {
    updateGitShowTimer = null;
    renderTerminalOutput(gitShow, data.git_show);
}
let renderTerminalOutputEpoch = 0;
function renderTerminalOutput(pre, text) {
    renderTerminalOutputEpoch += 1;
    pre.innerHTML = "";
    renderTerminalOutputTick(renderTerminalOutputEpoch, pre, text, 0);
}
function renderTerminalOutputTick(epoch, pre, text, startIndex) {
    if (epoch != renderTerminalOutputEpoch) {
        console.warn("renderTerminalOutput() cancelled");
        return;
    }
    const startTime = performance.now();
    // <https://en.wikipedia.org/w/index.php?title=ANSI_escape_code&oldid=1248130213#CSI_(Control_Sequence_Introducer)_sequences>
    const csiRuns = /\x1B\[[\x30-\x3F]*[\x20-\x2F]*[\x40-\x7E]|[^\x1B]+|[^]/g;
    csiRuns.lastIndex = startIndex;

    let runMatch;
    let fgColor = 0;
    while ((runMatch = csiRuns.exec(text)) != null) {
        const [run] = runMatch;
        const match = run.match(/\x1B\[([\x30-\x3F]*)[\x20-\x2F]*([\x40-\x7E])/);
        // branches labelled “optimise” significantly reduce the number of
        // nodes in the DOM tree, without any observable changes to rendering.
        // they can be removed if they turn out to be buggy.
        if (match) {
            const [, params, mode] = match;
            /* sgr: select graphic rendition */
            if (mode == "m") {
                for (const param of params.split(";")) {
                    const num = parseInt(param || "0", 10);
                    if (param.length > 0 && `${num}`.length != param.length) {
                        continue;
                    }
                    if (num == 0 || num >= 30 && num <= 37 || num >= 90 && num <= 97) {
                        fgColor = num;
                    }
                }
            }
        } else if (run == "\n" && pre.lastChild) {
            // optimise runs of only a newline, for example:
            // `\033[30m...\033[m\n\033[31m...`
            if (pre.lastChild.nodeName == "#text") {
                pre.lastChild.nodeValue += run;
            } else {
                pre.lastChild.firstChild.nodeValue += run;
            }
        } else if (fgColor != 0) {
            if (pre.lastChild?.dataset?.sgr == `${fgColor}`) {
                // optimise back-to-back runs of the same colour, for example:
                // `\033[30m...\033[30m...`
                pre.lastChild.firstChild.nodeValue += run;
            } else {
                const span = document.createElement("span");
                span.dataset.sgr = `${fgColor}`;
                span.style.color = `var(--sgr-${fgColor})`;
                span.append(run);
                pre.append(span);
            }
        } else if (fgColor == 0) {
            if (pre.lastChild?.nodeName == "#text") {
                // optimise back-to-back runs with no colour, for example:
                // `\033[m...\033[m...`
                pre.lastChild.nodeValue += run;
            } else {
                pre.append(run);
            }
        }
        // cap the render at a reasonable number of nodes.
        if (pre.childNodes.length >= 10000) {
            console.warn(`renderTerminalOutput() node limit exceeded: ${pre.childNodes.length} nodes, ${pre.textContent.split("\n").length}/${text.split("\n").length} lines`);
            pre.append("\n\n>>> renderTerminalOutput() node limit exceeded (see console)");
            return;
        }
        // render in chunks of up to 15ms at a time.
        // this does not include the time taken to reflow or paint.
        if (performance.now() - startTime > 15) {
            console.warn("renderTerminalOutput() time limit exceeded");
            requestAnimationFrame(() => void renderTerminalOutputTick(epoch, pre, text, csiRuns.lastIndex));
            return;
        }
    }
    console.log(`renderTerminalOutput() done: ${pre.childNodes.length} nodes, ${pre.textContent.split("\n").length}/${text.split("\n").length} lines`);
}
