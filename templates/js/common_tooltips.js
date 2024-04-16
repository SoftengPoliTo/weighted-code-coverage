/**
 * Common tooltips between `base.html` and `file_details.html`.
 */

tippy('.thresholds', {
    content: `
    <ul> 
        <li>- <b>Coverage</b>: is not used to determine whether a file is complex or not.</li>
        <li>- <b>Wcc</b>: is a percentage metric and, like coverage, it should be maximized.</li>
        <li>- <b>CRAP</b>: is a score and should be as low as possible.</li>
        <li>- <b>Skunk</b>: is a score and should be as low as possible.</li>
    </ul>`,
    allowHTML: true
});