import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

export default withMermaid(
  defineConfig({
    vite: {
      optimizeDeps: {
        include: ['mermaid'],
      },
    },
    title: 'Paraphrase',
    description: 'Pipeline orchestrator for data conversion',

    base: '/paraphase/',

    themeConfig: {
      nav: [
        { text: 'Philosophy', link: '/philosophy' },
        { text: 'Architecture', link: '/architecture-decisions' },
        { text: 'Rhizome', link: 'https://docs.rhi.zone/' },
      ],

      sidebar: {
        '/': [
          {
            text: 'Design',
            items: [
              { text: 'Philosophy', link: '/philosophy' },
              { text: 'Architecture Decisions', link: '/architecture-decisions' },
              { text: 'Use Cases', link: '/use-cases' },
              { text: 'Workflow API', link: '/workflow-api' },
              { text: 'Open Questions', link: '/open-questions' },
            ]
          },
        ]
      },

      socialLinks: [
        { icon: 'github', link: 'https://github.com/rhizome-lab/paraphase' }
      ],

      search: {
        provider: 'local'
      },

      editLink: {
        pattern: 'https://github.com/rhizome-lab/paraphase/edit/master/docs/:path',
        text: 'Edit this page on GitHub'
      },
    },
  }),
)
