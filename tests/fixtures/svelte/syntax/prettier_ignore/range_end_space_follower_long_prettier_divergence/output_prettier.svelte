<!-- A space after the end marker keeps the follower on the marker's line at exactly 100 and
breaks at 101: a component, then a text -->
<!-- prettier-ignore-start --><b >text1 text2 text3 text4xx</b><!-- prettier-ignore-end -->
<Comp />

<!-- prettier-ignore-start --><b >text1 text2 text3 text4xxx</b><!-- prettier-ignore-end -->
<Comp />

<!-- prettier-ignore-start --><b >text1 text2 text3 text4xxxxx</b><!-- prettier-ignore-end --> text5

<!-- prettier-ignore-start --><b >text1 text2 text3 text4xxxxxx</b><!-- prettier-ignore-end -->
text5
