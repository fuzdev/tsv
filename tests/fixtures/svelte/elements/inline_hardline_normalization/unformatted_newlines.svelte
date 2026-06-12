<!-- Text only: leading newline collapses to space -->
<a>
    text</a>

<!-- Text + inline child: leading newline collapses to space -->
<a>
    text<span>text</span></a>

<!-- Inline child only: leading newline collapses to space -->
<a>
    <span>text</span></a>

<!-- Text only: trailing newline collapses to space -->
<a>text
</a>

<!-- Text + inline child: trailing newline collapses to space -->
<a>text<span>text</span>
</a>

<!-- Inline child only: trailing newline collapses to space -->
<a><span>text</span>
</a>
