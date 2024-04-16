/**
 * Tooltips of `base.html`.
 */

tippy('.analysis-parameters-files', {
    content: `
    <ul> 
        <li>- <b>Mode</b>: <em>Files</em> mode shows only files metrics, instead <em>Functions</em> one shows also functions metrics.</li>
        <li>- <b>Complexity</b>: select to view metrics computed with <em>Cyclomatic</em> or <em>Cognitive</em> complexity.</li>
    </ul>`,
    allowHTML: true
});

tippy('.coverage-project', {
    content: `
    <ul> 
        <li>- <b>Overall</b>: percentage of covered lines of the entire project.</li>
        <li>- <b>Min</b>: minimum <em>coverage</em> among project files.</li>
        <li>- <b>Max</b>: maximum <em>coverage</em> among project files.</li>
        <li>- <b>Average</b>: average <em>coverage</em> of project files.</li>
    </ul>`,
    allowHTML: true
});

tippy('.wcc-project', {
    content: `<b>Wcc</b> is computed starting from coverage value, adjusting it according to complexity; indeed, its value is at most equal to coverage.
    <br><br>
    <ul> 
        <li>- <b>Overall</b>: is computed using project overall <em>PLOC</em> lines and coverage.</li>
        <li>- <b>Min</b>: minimum <em>Wcc</em> among project files.</li>
        <li>- <b>Max</b>: maximum <em>Wcc</em> among project files.</li>
        <li>- <b>Average</b>: average <em>Wcc</em> of project files.</li>
    </ul>`,
    allowHTML: true
});

tippy('.crap-project', {
    content: `<b>CRAP</b> is computed starting from complexity and combining it with coverage. Compared to <em>Wcc</em>, it is more influenced by complexity than coverage.
    <br><br>
    <ul> 
        <li>- <b>Overall</b>: is computed using overall project coverage and average complexity among all code spaces.</li>
        <li>- <b>Min</b>: minimum <em>CRAP</em> among project files.</li>
        <li>- <b>Max</b>: maximum <em>CRAP</em> among project files.</li>
        <li>- <b>Average</b>: average <em>CRAP</em> of project files.</li>
    </ul>`,
    allowHTML: true
});

tippy('.skunk-project', {
    content: `<b>Skunk</b> is computed starting from complexity and combining it with coverage. It is more influenced by complexity than coverage compared to <em>Wcc</em>, and less accurate than <em>CRAP</em>.
    <br><br>
    <ul> 
        <li>- <b>Overall</b>: is computed using overall project coverage and average complexity among all code spaces.</li>
        <li>- <b>Min</b>: minimum <em>Skunk</em> among project files.</li>
        <li>- <b>Max</b>: maximum <em>Skunk</em> among project files.</li>
        <li>- <b>Average</b>: average <em>Skunk</em> of project files.</li>
    </ul>`,
    allowHTML: true
});

tippy('.files', {
    content: `
    <ul> 
        <li>- <b>Not complex</b>: file for which <em>Wcc</em> is above the threshold and <em>CRAP</em> and <em>Skunk</em> are below the threshold.</li>
        <li>- <b>Complex</b>: file for which <em>Wcc</em> is below the threshold and/or at least one of <em>CRAP</em> and <em>Skunk</em> is above the threshold.</li>
        <li>- <b>Ignored</b>: file that lacks profiling and/or coverage information.</li>
    </ul>`,
    allowHTML: true
});

tippy('.file-table', {
    content: 'A file is considered <b>complex</b> if at least one of the following apply: <em>Wcc</em> below threshold, <em>CRAP</em> or <em>Skunk</em> above threshold.',
    allowHTML: true
});

tippy('.complexity-table', {
    content: 'Average <b>complexity</b> among file code spaces. This value is used in the computation of file <em>CRAP</em> and <em>Skunk</em>.',
    allowHTML: true
});

tippy('.coverage-table', {
    content: 'File overall <b>coverage</b>.',
    allowHTML: true
});

tippy('.wcc-table', {
    content: '<b>Wcc</b> is computed starting from coverage value, adjusting it according to complexity; indeed, its value is at most equal to coverage.',
    allowHTML: true
});

tippy('.crap-table', {
    content: `<b>CRAP</b> is computed by considering complexity and combining it with the coverage. Compared to <em>Wcc</em>, it is more influenced by complexity than coverage.
    <br>
    The complexity value used for file <b>CRAP</b> computation is the average complexity of file code spaces.
    `,
    allowHTML: true
});

tippy('.skunk-table', {
    content: `<b>Skunk</b> is computed starting from complexity and combining it with coverage. It is more influenced by complexity than coverage compared to <em>Wcc</em>, and less accurate than <em>CRAP</em>.
    <br>
    The complexity value used for file <b>Skunk</b> computation is the average complexity of file code spaces.
    `,
    allowHTML: true
});