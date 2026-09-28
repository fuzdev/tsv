<!-- Text glued to a range's end marker: the marker and the first word stay on the line the frozen
slice ends on, and the run wraps at its first space. The whole line fits at exactly 100; at 101
the last word wraps -->
<!-- prettier-ignore-start --><b >text1 text2 text3 text4</b><!-- prettier-ignore-end -->text5 text6

<!-- prettier-ignore-start --><b >text1 text2 text3 text4x</b><!-- prettier-ignore-end -->text5 text6

<!-- A longer run wraps again, with content glued before the range too -->
text0<!-- prettier-ignore-start --><b >text1 text2 text3</b><!-- prettier-ignore-end -->text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15 text16 text17 text18

<!-- A frozen slice past the print width: the glued first word still has no break point -->
<!-- prettier-ignore-start --><b >text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11</b><!-- prettier-ignore-end -->text12 text13
