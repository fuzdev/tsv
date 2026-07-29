<!-- Trailing text after the end marker: the seam keeps the authored single newline -->
<!-- prettier-ignore-start -->
<div      data-attr="value1"   ></div>
<!-- prettier-ignore-end -->
text1 text2
<div>block1</div>

<!-- An authored blank after the end marker is preserved -->
<!-- prettier-ignore-start -->
<div      data-attr="value2"   ></div>
<!-- prettier-ignore-end -->

text3
<div>block2</div>

<!-- An element straight after the end marker: no blank -->
<!-- prettier-ignore-start -->
<div      data-attr="value3"   ></div>
<!-- prettier-ignore-end -->
<div>block3</div>

<!-- Trailing text as the document's last node: no blank -->
<!-- prettier-ignore-start -->
<div      data-attr="value4"   ></div>
<!-- prettier-ignore-end -->
text4
