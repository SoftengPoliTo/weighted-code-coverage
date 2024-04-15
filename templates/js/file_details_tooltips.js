/**
 * Tooltips of `file_details.html`.
 */

tippy('.analysis-parameters-details', {
    content: `
    <ul> 
        <li>- <b>File</b>: selected file.</li>
        <li>- <b>Mode</b>: <em>Files</em> mode shows only files metrics, instead <em>Functions</em> one shows also functions metrics.</li>
        <li>- <b>Complexity</b>: select to view metrics computed with <em>Cyclomatic</em> or <em>Cognitive</em> complexity.</li>
    </ul>`,
    allowHTML: true
});

tippy('.file-metrics', {
    content: `
    <ul> 
        <li>- <b>Complexity</b>: average <em>complexity</em> among file code spaces.</li>
        <li>- <b>Coverage</b>: file overall <em>coverage</em>.</li>
        <li>- <b>Wcc</b>: file overall <em>Wcc</em>.</li>
        <li>- <b>CRAP</b>: file overall <em>CRAP</em>.</li>
        <li>- <b>Skunk</b>: file overall <em>Skunk</em>.</li>
    </ul>`,
    allowHTML: true
});

tippy('.functions', {
    content: `
    <ul> 
        <li>- <b>Not complex</b>: functions for which <em>Wcc</em> is above the threshold and <em>CRAP</em> and <em>Skunk</em> are below the threshold.</li>
        <li>- <b>Complex</b>: functions for which <em>Wcc</em> is below the threshold and/or at least one of <em>CRAP</em> and <em>Skunk</em> is above the threshold.</li>
    </ul>`,
    allowHTML: true
});

tippy('.function-table', {
    content: 'A function is considered <b>complex</b> if at least one of the following apply: <em>Wcc</em> below threshold, <em>CRAP</em> or <em>Skunk</em> above threshold.',
    allowHTML: true
});

tippy('.complexity-table', {
    content: 'Function <b>complexity</b> value.',
    allowHTML: true
});

tippy('.coverage-table', {
    content: 'Function <b>coverage</b> value.',
    allowHTML: true
});

tippy('.wcc-table', {
    content: '<b>Wcc</b> is computed starting from coverage value, adjusting it according to complexity; indeed, its value is at most equal to coverage.',
    allowHTML: true
});

tippy('.crap-table', {
    content: `<b>CRAP</b> is computed by considering complexity and combining it with the coverage. Compared to <em>Wcc</em>, it is more influenced by complexity than coverage.
    <br>
    The complexity value used for function <b>CRAP</b> computation is the complexity value of the function.
    `,
    allowHTML: true
});

tippy('.skunk-table', {
    content: `<b>Skunk</b> is computed starting from complexity and combining it with coverage. It is more influenced by complexity than coverage compared to <em>Wcc</em>, and less accurate than <em>CRAP</em>.
    <br>
    The complexity value used for function <b>Skunk</b> computation is the complexity value of the function.
    `,
    allowHTML: true
});