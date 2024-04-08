/**
 * Common tooltips between `base.html` and `file_details.html`
 */

tippy('#mode', {
    content: '<b>Files</b> shows only files metrics, instead <b>Functions</b> shows also functions metrics.',
    allowHTML: true
});

tippy('#complexity-type', {
    content: 'You can choose to visualize the metrics computed with <b>Cyclomatic</b> or <b>Cognitive</b> complexity.',
    allowHTML: true
});

tippy('#thresholds', {
    content: '<b>Wcc</b> is a metric expressed as a percentage and, like <b>Coverage</b>, it should be maximized. <b>CRAP</b> and <b>Skunk</b>, instead, are scores and should be as low as possible.',
    allowHTML: true
});

tippy('.wcc', {
    content: '<b>Wcc</b> is computed based on the coverage value, adjusting it according to complexity; indeed, the value of Wcc is at most equal to coverage. ',
    allowHTML: true
});

tippy('.crap', {
    content: '<b>CRAP</b> is computed by considering complexity and combining it with the coverage. Compared to Wcc, it is more influenced by complexity than coverage.',
    allowHTML: true
});

tippy('.skunk', {
    content: '<b>Skunk</b> is computed by considering complexity and combining it with the coverage. Compared to Wcc, it is more influenced by complexity than coverage.',
    allowHTML: true
});

tippy('#file-complexity', {
    content: 'Average <b>complexity</b> value of file code spaces.',
    allowHTML: true
});

tippy('#file', {
    content: 'A <b>file</b> is considered complex if at least one of the following apply: <b>Wcc</b> below threshold, <b>CRAP</b> or <b>Skunk</b> above threshold.',
    allowHTML: true
});

tippy('#function', {
    content: 'A <b>function</b> is considered complex if at least one of the following apply: <b>WCC</b> below threshold, <b>CRAP</b> or <b>Skunk</b> above threshold.',
    allowHTML: true
  });