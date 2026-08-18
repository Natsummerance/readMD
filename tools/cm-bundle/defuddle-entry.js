import Defuddle from 'defuddle/full';

window.ReadMDDefuddle = {
  parse(document, url) {
    const extractor = new Defuddle(document, {
      url,
      markdown: true,
      separateMarkdown: true,
      useAsync: false,
    });
    const result = extractor.parse();
    // Defuddle 0.19 may place Markdown in `content` when `markdown` is true.
    if (result && !result.contentMarkdown && result.content) {
      result.contentMarkdown = result.content;
    }
    return result;
  },
};
