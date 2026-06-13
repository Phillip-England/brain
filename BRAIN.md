<!-- project: brain | created_at: 2026-06-13T15:43:25.478228Z | id: 4be208fa-9263-405e-81b1-a75159dbbb36 -->

# the site should be mobile friendly

this is intended to be used on a phone to collect ideas about certain projects while a user is on the go let's imagine that a user is on the go and they want to log into this portal and then they select a specific project that they're working on and they can collect ideas about that specific project and then essentially save them for later so that is something that we want to make sure that this is very mobile friendly to use

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.478148Z | id: dade3cb4-13c6-4cd5-8f49-34ba24464d26 -->

# the system interacts with you while you speak

in the say system we don't have any data for us to scan while we are speaking into the system but in the brain system we have a whole bunch of markdown files on disc that we can scan so here's kind of the idea while the user is speaking and creating their markdown file behind the scenes the brain system is checking to see if any ideas relate to the current I did the user is speaking and these kind of pop up in the user interface as they are speaking now they have to pop up in a way that is not invasive because the main idea of the page that is recording a user's idea is all about visually seeing the markdown file such as it is in the say system but in the brain system we should be able to visually see that hey this idea that you're currently speaking has keywords that relates to these other ideas that you've spoken before and this should be in a non-invasive way so while the user is speaking they should be able to sort of see that hey this relates to other ideas that you spoken before do you maybe just want to use those ideas instead of creating a new one

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.478062Z | id: 16200a63-fee5-471c-8a9d-b913da9c2772 -->

# this is a searchable system

the brain system is intended to be a searchable system the user should be able to search and scan all of the ideas in all of the projects and look through them all and find different ideas and then import them into their specific project and so that is the whole idea

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.477978Z | id: 8a7101b0-612c-48e2-8b20-4115dd71176e -->

# the user should be able to share markdown files or ideas between projects

let's imagine a situation where a user creates a new project but they know that they are going to use ideas that have already been spoken into a previously existing project while the user interface should make it very easy for the user to essentially import ideas from other projects into the currently selected project

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.477892Z | id: 72d4f603-90f1-4697-9de7-729243785eba -->

# the user will use their voice to enter markdown files into the system

this logic is already built out in another application called say and the source code for say will be included in this so that you understand how the user will input data into the brain system for a particular project

essentially the save system allows users to enter markdown files and create them using their voice you can make use of the source code from the system and integrate its ideas into this project the user is not going to be uploading markdown files into the brain system nor will the user be manually typing them out instead they will be adding markdown files to the system using their voice and the say system already has that logic and user interface and everything built out for you to be able to copycat in this project

the difference between the say system and the Brain system that you are currently creating is that the say system does not allow for the user to actually save those markdown files in associate them with a certain project instead they just copy the markdown files to their clipboard but in your brain system whenever the user creates a markdown file they will have a special button that they can press to actually save that markdown file and Associate it with the current project that they created it beneath so that is how you're going to create the user interface for the user to be able to enter markdown files into the system they won't be uploaded and they won't be manually typed in they will be spoken into the system using the user's voice and they will be associated with a certain project so the user will select a project then they can see all the ideas for that project that have already been created which are just marked on files and then the user can create new markdown files

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.477790Z | id: 42407fb3-bff2-494b-bb58-82952296dd7d -->

# ideas are essentially just marked down files that are structured in a specific way

when a user adds an idea to the system they are essentially just adding text to the system but the markdown file is structured in a very specific way it can only have headers and paragraphs so an idea is just a markdown file with headers and paragraphs in it and the first thing in the markdown file must be a header with a single hashtag if you try to enter a marked on file into the system that does not start with a single header hashtag then it will be denied because that is how the file is named okay also whenever I marked on file is entered into the system it's going to have some metadata because we need to know what time it was entered into the system because the user is going to be able to search through these markdown files or see them in order alphabetically or see them in order by the time they were entered

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.477298Z | id: da46f4c1-96a3-4da2-898c-e869042bd2e9 -->

# the admin can create projects

the admin can create new projects so the brain system is oriented around projects and ideas associated with those projects okay so we can create a new project and a new project is essentially just a directory on the system so we have a specific folder the brain folder and it is located in a particular spot that is designated by an environment variable and this is set up automatically for the user whenever they run the application but it is configurable behind the scenes in the admin portal in the settings section but that way the user knows where all of their files and projects are being stored but a project is essentially just a subdirectory within the brain directory so the admin is able to create these projects name them rename them that sort of thing and delete them whenever the admin deletes the project it deletes all the files within the project directory so they need to type out and say yes I want to delete that

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.477212Z | id: c353392d-ba13-4abb-b6c5-62ee3a86c2b7 -->

# we need built in IP Banning for login abuse

we're going to make use of SQL light to track bad login attempts every time a bad login attempt occurs we are going to log it in the SQL light database but we also want to check the database to make sure that there are no entries that are older than 24 hours every time somebody attempts to log in that way the database is self purging is automatically deleting any entries that are older than 24 hours on every login attempt this way we can figure out and use the information the database to determine if somebody is abusing the login form or not if somebody is determined to be used abusing the login form then we will IP ban them so they can no longer log into the website

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.477108Z | id: 0dc205e3-f6eb-4627-9685-d785eae6fd24 -->

# the command line interface should make setting the admins username and password very easy and straightforward

there should be a command designated in the readme file or I should be able to run a help command to be able to see all the available commands on the system and there should be a command to set my username and password very easily and this should work on Windows Linux Mac any platform that I would want to install this application

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.476997Z | id: 1d07248c-bea7-489f-9e30-9cfa54b00ce7 -->

# this project lives online and has a single user that's an admin

the credentials for the admin are set using environment variables on the system not a file but the environment variables for the global operating system we want to make sure to specify that

---

<!-- project: brain | created_at: 2026-06-13T15:43:25.476780Z | id: a615f803-b7ae-4d59-8d10-38deeea99aca -->

# this project is written in Rust

the reason is because rust is easy to install applications as a single binary

---

<!-- project: brain | created_at: 2026-06-13T15:38:30.336276Z | id: b21395b2-50d2-4f45-ae2c-d046e1a2de68 -->

# I should be able to select ideas and delete multiple ideas at once

this would be super useful in the event that I accidentally add ideas to the system need to delete multiple ideas

---

<!-- project: brain | created_at: 2026-06-13T15:30:41.253400Z | id: 89ca2842-92db-4606-b9a0-0a9dccdd2090 -->

# when I copy ideas onto my clipboard all of the ideas should become unselected

I find that right now whenever I go back to the ideas page I see these selected ideas but I noticed that whenever I copy them they should automatically be unselected because I'm having to go back and manually unselect them and that is not fun

---

<!-- project: brain | created_at: 2026-06-13T15:29:56.636384Z | id: e21f55fd-1385-4f34-941d-79b441377883 -->

# when I record an idea I should be able to record multiple ideas in one go

let's imagine I'm on a roll and I want to record multiply ideas in one go where well I should be able to do that as long as it is headers separated by paragraphs of text and I can have multiple paragraphs of text per idea as long as that is the case then the idea should be accepted by the system and whenever I say it should still go on my clipboard that would be amazing if I could save multiple ideas at once

---

<!-- project: brain | created_at: 2026-06-13T15:28:46.291994Z | id: 5e939b38-bd28-4ffa-8770-9edb7a163731 -->

# when I save an idea that idea should automatically go on the clipboard

I noticed that I'm having the frequently go to my ideas and grab them to put them on my clipboard I should be able to just do that automatically whenever I save an idea it should automatically go in the clipboard

---

<!-- project: brain | created_at: 2026-06-13T15:28:11.349939Z | id: b5a9b8c5-560a-4182-bb9e-16742f646adc -->

# I should be able to import text not just a markdown file

right now I can only import a markdown file into a project but imagine I have the text on my clipboard and want to go ahead and put that into the system as well I should be able to do that

---

<!-- project: brain | created_at: 2026-06-13T15:26:31.871531Z | id: 2138bccd-0e3b-42c6-9c3c-5ddfce02fb05 -->

# I should have the ability to select all ideas in a project

right now I'm not sure I can select all ideas in a project and it would be really useful to be able to do that

---

<!-- project: brain | created_at: 2026-06-13T15:21:04.871248Z | id: fae71e84-0fa1-40c8-b896-ef970396e3d9 -->

# the ability to import markdown files

imagine I have a markdown file and the markdown file is structured such that it has headers with the single hashtags separated by paragraphs of text if this is the case where we only have headers with single hashtags and the markdown file starts with a header with a single hashtag and there is Not Duplicate headers in a row where we have one header followed by another without text separating them if the markdown file fits all of that criteria then you can upload it into the system and Associate it with a specific project so let's imagine I have selected a specific project then I can choose to upload and markdown file and then it will separate all of those ideas into their own ideas in the system if that makes sense so this gives the system a way to accept a markdown file the system will split it into multiple ideas and then upload them into the system that allows the user to upload markdown files in that way as well

---

<!-- project: brain | created_at: 2026-06-13T15:19:35.852722Z | id: 84f6aad2-5db3-4750-aaeb-c27eed9614ad -->

# I should be able to press the U button to undo

right now I have to click the undo button and I should be able to press the U button to do that

---

<!-- project: brain | created_at: 2026-06-13T15:19:19.337034Z | id: 2739d686-4981-443a-836c-ecec1b5b4a73 -->

# please review the ideas in this system and make sure they match the appropriate format

I have already mentioned the ideas in the system have to be a single title and then text thereafter I need to review all the ideas in the system and make sure that they are appropriately formatted if they are not deleted

---

<!-- project: brain | created_at: 2026-06-13T15:17:19.353825Z | id: 06b8d314-5aa6-49aa-9e38-2ad5dc3d02a8 -->

# what is an idea in this system

I want to be very clear about this an idea in this system is simply one header that starts with a hashtag and then it can be multiple paragraphs after that but that is a single idea in this system so a header followed by multiple paragraphs or one paragraph of text so for example if some if you try to upload multiple headers in one upload that will be rejected by the system this system only accepts a single header and that header only can have one hashtag and then it accepts this much text as you want after that but only paragraphs of text nothing more

---

<!-- project: brain | created_at: 2026-06-13T15:03:23.220556Z | id: 8241ca48-1cb6-4aa2-a7f6-18d2a2b36f12 -->

# this project is written in Rust

rust is easy to install and that is one of the main driving points of choosing rust because I can install a single binary for the application