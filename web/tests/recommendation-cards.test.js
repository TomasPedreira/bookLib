import test from 'node:test';
import assert from 'node:assert/strict';
import { JSDOM } from 'jsdom';
import { hasRecommendationTitle, homeRecommendationCard } from '../recommendation-cards.js';

function inspect(item, check, preferred = []) {
  const dom = new JSDOM(homeRecommendationCard(item, 7, preferred, '<span class="home-cover">No cover</span>'));
  try { check(dom.window.document); }
  finally { dom.window.close(); }
}

test('recommendations keep one-letter and long titles but exclude empty titles', () => {
  assert.equal(hasRecommendationTitle({title:' X '}), true);
  assert.equal(hasRecommendationTitle({title:'W'.repeat(2000)}), true);
  for (const title of [null, undefined, '', ' \n ', 42]) assert.equal(hasRecommendationTitle({title}), false);
});

test('missing metadata has explicit labels and a clickable footer', () => {
  inspect({title:'X', authors:[null, ' '], genres:[null, ' ', 'not_a_topic']}, document => {
    assert.equal(document.querySelector('.home-recommend-author').textContent, 'Unknown author');
    assert.equal(document.querySelector('.home-recommend-rating').textContent, 'No ratings');
    const footer = document.querySelector('.home-recommend-footer');
    assert.equal(footer.textContent, 'No topics listed');
    assert.equal(footer.closest('button').dataset.homeRecommend, '7');
  });
});

test('long and hostile metadata stays literal and available in book details', () => {
  const title = '<img src=x onerror=alert(1)>' + 'W'.repeat(2000);
  const author = '<script>wrong()</script>' + 'A'.repeat(1000);
  inspect({title, authors:[null, author, ' '], genres:[]}, document => {
    assert.equal(document.querySelector('.home-recommend-info > strong').textContent, title);
    assert.equal(document.querySelector('.home-recommend-info > strong').title, title);
    assert.equal(document.querySelector('.home-recommend-author').textContent, author);
    assert.equal(document.querySelector('.home-recommend-author').title, author);
    assert.equal(document.querySelectorAll('script, img').length, 0);
  });
});

test('topic footer deduplicates, prioritizes selected genres and preserves hidden topics', () => {
  inspect({title:'Many worlds', genres:['fantasy', 'mystery', 'historical_fiction', 'fantasy', 'science', 'toString'], genre:'science'}, document => {
    const footer = document.querySelector('.home-topics');
    assert.equal(footer.querySelector('strong').textContent, 'Science');
    assert.equal(footer.querySelector('.home-topic-more').textContent, '+1');
    assert.equal(footer.title, 'Science, Fantasy, Mystery, Historical fiction');
    assert.equal(JSON.parse(footer.dataset.homeTopics).length, 4);
  }, ['science']);
});

test('ratings never show NaN, invalid values or an unsupported popularity claim', () => {
  for (const ratings_average of [null, undefined, NaN, Infinity, -1, 6, '4.2']) {
    inspect({title:'A book', ratings_average, ratings_count:10}, document => assert.equal(document.querySelector('.home-recommend-rating').textContent, 'No ratings'));
  }
  inspect({title:'A book', ratings_average:4.24, ratings_count:10}, document => assert.equal(document.querySelector('.home-recommend-rating').textContent, '★ 4.2'));
  inspect({title:'A book', ratings_average:4.24, ratings_count:2}, document => assert.equal(document.querySelector('.home-recommend-rating').textContent, 'No ratings'));
});
