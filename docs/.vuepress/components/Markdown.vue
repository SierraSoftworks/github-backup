<template>
    <span v-html="compiledHtml" />
</template>

<script lang="ts">
import MarkdownIt from 'markdown-it'
import { defineComponent, computed, toRef } from 'vue'

export default defineComponent({
    props: {
        value: String,
        inline: {
            type: Boolean,
            required: false,
            default: false
        }
    },
    setup(props) {
        const converter = new MarkdownIt({
            html: true,
            linkify: true
        });

        const value = toRef(props, 'value')
        const nonNullValue = computed(() => value.value || '')

        const compiledHtml = computed(() => props.inline ? converter.renderInline(nonNullValue.value) : converter.render(nonNullValue.value))

        return {
            compiledHtml
        }
    },
})
</script>
