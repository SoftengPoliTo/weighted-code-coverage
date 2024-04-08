// Navbar burger handler.
document.addEventListener("DOMContentLoaded", () => {
    const $navbarBurgers = Array.prototype.slice.call(
        document.querySelectorAll(".navbar-burger"),
        0
    );

    if ($navbarBurgers.length > 0) {
        $navbarBurgers.forEach((el) => {
            el.addEventListener("click", () => {
                const target = el.dataset.target;
                const $target = document.getElementById(target);

                el.classList.toggle("is-active");
                $target.classList.toggle("is-active");
            });
        });
    }
});

// Close hamburger menu on complexity selection.
function closeHamburger() {
    var navbarBurgers = document.getElementsByClassName("navbar-burger");
    for (let e of navbarBurgers) {
        const target = e.dataset.target;
        const $target = document.getElementById(target);
        e.classList.remove("is-active");
        $target.classList.remove("is-active");
    }
}

// Handle switch to cyclomatic complexity.
function cyclomatic() {
    chart.data.datasets[0].data[0] = notComplexCyclomatic;
    chart.data.datasets[0].data[1] = complexCyclomatic;
    chart.update();

    var cyclomaticElements = document.getElementsByClassName("cyclomatic");
    for (let e of cyclomaticElements) {
        e.classList.remove("is-hidden");
    }

    var cognitiveElements = document.getElementsByClassName("cognitive");
    for (let e of cognitiveElements) {
        e.classList.add("is-hidden");
    }

    closeHamburger();
}

// Handle switch to cognitive complexity.
function cognitive() {
    chart.data.datasets[0].data[0] = notComplexCognitive;
    chart.data.datasets[0].data[1] = complexCognitive;
    chart.update();

    var cognitiveElements = document.getElementsByClassName("cognitive");
    for (let e of cognitiveElements) {
        e.classList.remove("is-hidden");
    }

    var cyclomaticElements = document.getElementsByClassName("cyclomatic");
    for (let e of cyclomaticElements) {
        e.classList.add("is-hidden");
    }

    closeHamburger();
}