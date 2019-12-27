-- Your SQL goes here
SET NAMES utf8;

CREATE DATABASE `gusyara` /*!40100 DEFAULT CHARACTER SET utf8mb4 */;

CREATE USER 'gusyara'@'%' IDENTIFIED BY '12345';
GRANT ALL PRIVILEGES ON gusyara.* TO 'gusyara'@'%';

FLUSH PRIVILEGES;

USE `gusyara`;

DROP TABLE IF EXISTS `videos`;
CREATE TABLE `videos` (
  `id` int(10) unsigned NOT NULL AUTO_INCREMENT,
  `name` varchar(50) NOT NULL,
  `createdat` datetime DEFAULT NULL,
  PRIMARY KEY (`id`)
);