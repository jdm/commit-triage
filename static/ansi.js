let renderTerminalOutputEpoch = 0;

export function renderTerminalOutput(pre, text) {
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
