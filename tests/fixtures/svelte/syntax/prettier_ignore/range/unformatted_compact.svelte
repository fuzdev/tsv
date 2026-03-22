<script>
const a=true;
</script>

<div>text</div>

<!-- prettier-ignore-start -->

<div      > text1 text2 </div>

<p>
    text3
    text4
</p>

<!-- prettier-ignore-end -->

<div>text</div>

<!-- prettier-ignore-start -->

<span   data-attr="value"   >  text5  </span>

<!-- prettier-ignore-end -->

<!-- nested inside element - does NOT activate ignore range -->
<div>
<!-- prettier-ignore-start -->

<div>text6 text7</div>

<p>text8</p>

<!-- prettier-ignore-end -->
</div>

<div>text</div>
