<style>
/* Short complex selectors - combinators stay inline (under 100 chars per selector) */
.a > .b + .c ~ .d,.e > .f .g:hover,.h[data-x] > .i:first-child::before { color: red; }

/* Long complex selectors - test combinator wrapping (>, +, ~, space) */
.class1 > .class2:not(.class3):hover > .class4 + .class5 ~ .class6[data-attr1][data-attr2]:first-child,#id1.class7.class8.class9 > div.class10:nth-child(2n + 1) span.class11:not(.class12):focus-within::before,article > section.class13 > div.class14 > p.class15 > span.class16:where(.class17, .class18)::after { color: red; }
</style>
