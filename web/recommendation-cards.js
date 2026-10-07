import { popularTopics, topicLabel } from './topics.js';

const text = value => typeof value === 'string' ? value.trim() : '';
const escape = value => String(value).replace(/[&<>"']/g, character => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[character]));

export const hasRecommendationTitle = item => Boolean(text(item?.title));

function topicsMarkup(topics, count) {
  if (!topics.length) return '<span class="home-empty-topics">No topics listed</span>';
  const more = topics.length - count;
  return topics.slice(0, count).map((topic, index) => `<span class="home-topic">${index ? '<span class="home-topic-separator" aria-hidden="true">·</span>' : ''}${topic.preferred ? `<strong>${escape(topic.label)}</strong>` : escape(topic.label)}</span>`).join('')
    + (more ? `<span class="home-topic-more" aria-label="${more} more ${more === 1 ? 'topic' : 'topics'}">+${more}</span>` : '');
}

export function homeRecommendationCard(item, index, preferredGenres, cover) {
  const title = text(item.title);
  const authors = (Array.isArray(item.authors) ? item.authors.map(text).filter(Boolean).join(', ') : text(item.authors)) || 'Unknown author';
  const rating = typeof item.ratings_average === 'number' && Number.isFinite(item.ratings_average) && item.ratings_average >= 0 && item.ratings_average <= 5 && item.ratings_count >= 5
    ? `<span aria-hidden="true">★</span> ${item.ratings_average.toFixed(1)}` : 'No ratings';
  const topics = [...new Set([...(Array.isArray(item.genres) ? item.genres : []), item.genre].filter(topic => Object.prototype.hasOwnProperty.call(popularTopics, topic)))]
    .sort((a, b) => Number(preferredGenres.includes(b)) - Number(preferredGenres.includes(a)))
    .map(key => ({label:topicLabel(key), preferred:preferredGenres.includes(key)}));
  const topicText = topics.map(topic => topic.label).join(', ') || 'No topics listed';
  return `<article class="home-recommend-card"><button class="home-recommend-main" type="button" data-home-recommend="${index}" aria-label="Open details for ${escape(title)}"><span class="home-recommend-body">${cover}<span class="home-recommend-info"><strong dir="auto" title="${escape(title)}">${escape(title)}</strong><span class="home-recommend-author" dir="auto" title="${escape(authors)}">${escape(authors)}</span><small class="home-recommend-rating">${rating}</small></span></span><span class="home-recommend-footer"><span class="home-topics${topics.length <= 1 ? ' single-topic' : ''}" data-home-topics="${escape(JSON.stringify(topics))}" title="${escape(topicText)}" aria-label="Book topics: ${escape(topicText)}">${topicsMarkup(topics, Math.min(3, topics.length))}</span></span></button></article>`;
}

export function fitRecommendationTopics(root) {
  for (const footer of root.querySelectorAll('[data-home-topics]')) {
    if (!footer.clientWidth) continue;
    const topics = JSON.parse(footer.dataset.homeTopics);
    let count = Math.min(3, topics.length);
    do {
      footer.classList.toggle('single-topic', count <= 1);
      footer.innerHTML = topicsMarkup(topics, count);
      if (footer.scrollWidth <= footer.clientWidth + 1 || count <= 1) break;
      count--;
    } while (count);
  }
}
