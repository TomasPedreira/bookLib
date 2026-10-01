export const topicGroups = [
  {label:'Lives & society', topics:{biography:'Biography',memoir:'Memoir',autobiography:'Autobiography',history:'History',politics:'Politics',philosophy:'Philosophy',psychology:'Psychology',religion:'Religion'}},
  {label:'Fiction & stories', topics:{fiction:'Fiction',classics:'Classics',fantasy:'Fantasy',science_fiction:'Sci-fi',romance:'Romance',mystery:'Mystery',thriller:'Thriller',crime:'Crime',horror:'Horror',historical_fiction:'Historical fiction',dystopian:'Dystopian',adventure:'Adventure',short_stories:'Short stories',literary_fiction:'Literary fiction',humor:'Humor'}},
  {label:'Knowledge & interests', topics:{nonfiction:'Nonfiction',science:'Science',technology:'Technology',business:'Business',finance:'Finance',self_help:'Self-help',health:'Health',cooking:'Cooking',art:'Art',travel:'Travel',sports:'Sports',education:'Education',music:'Music'}},
  {label:'Readers & formats', topics:{young_adult:'Young adult',children:'Children’s',comics:'Comics',graphic_novels:'Graphic novels',manga:'Manga',poetry:'Poetry'}}
];
export const popularTopics = Object.assign({}, ...topicGroups.map(group => group.topics));
export const topicSearchAliases = {biography:'biografia',memoir:'memorias memórias',autobiography:'autobiografia',nonfiction:'nao ficcao não ficção',science_fiction:'ficcao cientifica ficção científica',graphic_novels:'novelas graficas novelas gráficas'};
export const languages = {eng:'English',por:'Portuguese',spa:'Spanish',fre:'French',ger:'German',ita:'Italian'};

export const topicLabel = key => popularTopics[key] || key.replaceAll('_', ' ');
